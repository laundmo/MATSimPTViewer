use std::sync::{Arc, Mutex};

mod schedule;
use crate::schedule::{Departure, SchedulePlugin, StopFacility};
use bevy::dev_tools::fps_overlay::FpsOverlayPlugin;
use bevy::prelude::*;
use bevy_egui::{EguiContexts, EguiPlugin, EguiPrimaryContextPass, EguiStartupSet, egui};
use walkers::{HttpTiles, Map, MapMemory, lat_lon, lon_lat, sources::OpenStreetMap};
use walkers::{Plugin as MapPlugin, Projector};

struct StopsMapPlugin<F>
where
    F: Fn(Entity) + Send + Sync + 'static,
{
    stop_positions: Vec<(Entity, StopFacility)>,
    on_stop_click: F,
}

impl<F> MapPlugin for StopsMapPlugin<F>
where
    F: Fn(Entity) + Send + Sync + 'static,
{
    fn run(
        self: Box<Self>,
        ui: &mut egui::Ui,
        _response: &egui::Response,
        projector: &Projector,
        _map_memory: &MapMemory,
    ) {
        for (stop_entity, stop) in &self.stop_positions {
            let screen_pos =
                projector.project(lon_lat(stop.lon.unwrap_or(0.0), stop.lat.unwrap_or(0.0)));
            let rect = egui::Rect::from_center_size(screen_pos.to_pos2(), egui::Vec2::splat(20.0));
            let button = egui::Button::new("🚉").min_size(egui::Vec2::splat(20.0));
            let response = ui.put(rect, button);
            if response.clicked() {
                (self.on_stop_click)(stop_entity.clone());
            }
        }
    }
}

#[derive(Component)]
struct StationDetails {
    station: Option<Entity>,
}

#[derive(Event)]
struct StationClickedEvent(Entity);

#[derive(Component)]
struct MapMemoryComponent {
    map_memory: MapMemory,
}

#[derive(Component)]
struct MapTiles {
    tiles: HttpTiles,
}

fn setup_camera(mut commands: Commands) {
    commands.spawn((Camera2d::default(),));
}

fn setup_map(contexts: EguiContexts, mut commands: Commands) {
    let egui_ctx = contexts.ctx().expect("Failed to get Egui context").clone();
    let tiles = HttpTiles::new(OpenStreetMap, egui_ctx);
    let map_memory = MapMemory::default();

    commands.spawn((MapMemoryComponent { map_memory }, MapTiles { tiles }));

    commands.spawn(StationDetails { station: None });
}

fn map_ui(
    contexts: EguiContexts,
    mut query: Query<(&mut MapTiles, &mut MapMemoryComponent)>,
    stop_query: Query<(Entity, &StopFacility)>,
    mut ev_station_clicked: EventWriter<StationClickedEvent>,
) {
    let egui_ctx = contexts.ctx().expect("Failed to get Egui context");
    let clicked_events = Arc::new(Mutex::new(Vec::new()));
    let clicked_events_for_closure = Arc::clone(&clicked_events);
    egui::CentralPanel::default().show(egui_ctx, |ui| {
        if let Ok((mut map_tiles, mut map_memory)) = query.single_mut() {
            let mut map = Map::new(
                Some(&mut map_tiles.tiles),
                &mut map_memory.map_memory,
                lat_lon(52.455040534960574, 13.509400840651349),
            )
            .with_plugin(StopsMapPlugin {
                stop_positions: stop_query.iter().map(|(e, s)| (e, s.clone())).collect(),
                on_stop_click: move |entity| {
                    clicked_events_for_closure.lock().unwrap().push(entity);
                },
            });

            let _response = map.show(ui, |_, _, _| {});
        }
    });
    for entity in clicked_events.lock().unwrap().iter() {
        ev_station_clicked.write(StationClickedEvent(*entity));
    }
}

fn on_station_clicked(
    mut ev_station_clicked: EventReader<StationClickedEvent>,
    mut query: Query<&mut StationDetails>,
) {
    for event in ev_station_clicked.read() {
        if let Ok(mut station_details) = query.single_mut() {
            station_details.station = Some(event.0);
        }
    }
}

fn departure_board(
    mut contexts: EguiContexts,
    mut station_details_query: Query<&mut StationDetails>,
    stop_query: Query<&StopFacility>,
    departure_query: Query<&Departure>,
) {
    if let Ok(station_details) = station_details_query.single() {
        if let Some(station_entity) = station_details.station {
            let ctx = contexts.ctx_mut().unwrap();
            if let Ok(station) = stop_query.get(station_entity) {
                egui::SidePanel::right("departure_board")
                    .resizable(true)
                    .default_width(300.0)
                    .show(ctx, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(format!(
                                "Departures for station:\n{}",
                                station.name.clone().unwrap_or("Unnamed".to_string())
                            ));
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Max), |ui| {
                                if ui.button("❌").clicked() {
                                    // Clear selected station
                                    if let Ok(mut station_details) =
                                        station_details_query.single_mut()
                                    {
                                        station_details.station = None;
                                    }
                                }
                            });
                        });
                        ui.separator();

                        let mut departures: Vec<&Departure> = station
                            .departures
                            .iter()
                            .filter_map(|e| departure_query.get(*e).ok())
                            .collect();
                        departures.sort_by_key(|d| d.departure_time);

                        egui::ScrollArea::vertical()
                            .auto_shrink(false)
                            .show(ui, |ui| {
                                for departure in &departures {
                                    ui.horizontal(|ui| {
                                        if let Some(dep_time) = departure.departure_time {
                                            ui.label(format!("{}", dep_time.format("%H:%M:%S")));
                                        } else {
                                            ui.label("unknown time");
                                        }
                                        ui.add_space(8.0);
                                        ui.label(format!(
                                            "{} -> {}",
                                            departure.line_name, departure.route_headsign
                                        ));
                                    });
                                }
                            });
                    });
            }
        }
    }
}

fn main() {
    App::new()
        // Configure settings with defaults
        .add_plugins(DefaultPlugins)
        .add_plugins(EguiPlugin::default())
        .add_plugins(FpsOverlayPlugin::default())
        .add_plugins(SchedulePlugin {
            path: "schedule.xml".to_string(),
        })
        .add_systems(
            PreStartup,
            setup_camera.before(EguiStartupSet::InitContexts),
        )
        .add_systems(Startup, setup_map.after(EguiStartupSet::InitContexts))
        .add_systems(EguiPrimaryContextPass, map_ui)
        .add_event::<StationClickedEvent>()
        .add_systems(Update, on_station_clicked)
        .add_systems(EguiPrimaryContextPass, departure_board)
        .run();
}
