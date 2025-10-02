use bevy::prelude::*;
use chrono::TimeDelta;
use chrono::prelude::*;
use proj::Proj;
use quick_xml::events::Event;
use quick_xml::reader::Reader;
use std::collections::HashMap;
use std::fs::File;
use std::io::BufReader;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum RouteType {
    Tram,
    Subway,
    Bus,
    Ferry,
    SuburbanRail,
    HighSpeedRail,
    RegionalRail,
    Other,
}

impl RouteType {
    fn from_int(value: u16) -> Option<Self> {
        match value {
            0 => Some(RouteType::Tram),
            1 => Some(RouteType::Subway),
            2 => Some(RouteType::RegionalRail),
            3 => Some(RouteType::Bus),
            4 => Some(RouteType::Ferry),
            100 => Some(RouteType::RegionalRail),
            101 => Some(RouteType::HighSpeedRail),
            106 => Some(RouteType::RegionalRail),
            109 => Some(RouteType::SuburbanRail),
            600 => Some(RouteType::Subway),
            700 => Some(RouteType::Bus),
            _ => Some(RouteType::Other),
        }
    }
}

#[derive(Component, Default, Debug, Clone)]
#[allow(dead_code)]
pub struct StopFacility {
    pub id: String,
    pub x: f64,
    pub y: f64,
    pub name: Option<String>,
    pub stop_area_id: Option<String>,
    pub lat: Option<f64>,
    pub lon: Option<f64>,
    pub routes: Vec<Entity>,     // relationship?
    pub lines: Vec<Entity>,      // relationship?
    pub departures: Vec<Entity>, // relationship?
}

#[derive(Component, Clone, Debug)]
#[allow(dead_code)]
pub struct Departure {
    pub line_name: String,
    pub route_headsign: String,
    pub arrival_time: Option<NaiveDateTime>,
    pub departure_time: Option<NaiveDateTime>,
    pub stop: Entity,
    pub trip: Entity,
    pub route: Entity,
}

#[derive(Component, Clone, Debug)]
#[allow(dead_code)]
struct Line {
    id: String,
    name: String,
    agency_id: Option<String>,
    route_type: Option<RouteType>,
    routes: Vec<Entity>,
    trips: Vec<Entity>,
}

#[derive(Component, Clone, Debug)]
struct Route {
    id: String,
    route_profile: Vec<Entity>,
    departures: Vec<Entity>,
    trips: Vec<Entity>,
    line: Entity,
}

#[derive(Component, Clone, Debug)]
#[allow(dead_code)]
struct Trip {
    id: String,
    route: Entity,
    line: Entity,
    departures: Vec<Entity>,
    headsign: Option<String>,
}

#[derive(Component, Clone, Debug)]
#[allow(dead_code)]
struct RouteDeparture {
    id: String,
    departure_time: NaiveDateTime,
    route: Entity,
    trip: Option<Entity>,
}

#[derive(Component, Clone, Debug)]
struct RouteProfilePoint {
    #[allow(dead_code)]
    route: Entity,
    stop: Entity,
    arrival_offset: TimeDelta,
    departure_offset: TimeDelta,
}

#[derive(Resource)]
pub struct Schedule {
    pub path: String,
    pub stop_facilities: HashMap<String, Entity>,
    pub lines: HashMap<String, Entity>,
    pub routes: Vec<Entity>,
    pub trips: Vec<Entity>,
    pub departures: Vec<Entity>,
}

impl Schedule {
    pub fn new(path: String) -> Self {
        Schedule {
            path,
            stop_facilities: HashMap::new(),
            lines: HashMap::new(),
            routes: Vec::new(),
            trips: Vec::new(),
            departures: Vec::new(),
        }
    }
}

pub struct SchedulePlugin {
    pub path: String,
}

impl SchedulePlugin {
    pub fn load(world: &mut World) {
        let path = world.get_resource::<Schedule>().unwrap().path.clone();
        info!("Loading schedule from XML");
        let file = File::open(path).expect("Failed to open schedule file");
        let file = BufReader::new(file);
        let mut reader = Reader::from_reader(file);
        reader.config_mut().trim_text(true);

        let mut buf = Vec::new();

        let mut in_line = false;
        let mut in_route = false;
        let mut in_line_attributes = false;
        let mut in_route_profile = false;
        let mut in_departures = false;
        let mut after_line_attributes = false;

        let mut current_line: Option<String> = None;
        let mut current_line_name: Option<String> = None;
        let mut current_line_route_type: Option<RouteType> = None;
        let mut current_line_agency_id: Option<String> = None;

        let mut current_line_entity: Option<Entity> = None;

        let mut current_route: Option<String> = None;
        let mut current_route_entity: Option<Entity> = None;

        let mut current_attribute_name: Option<String> = None;

        let proj = Proj::new_known_crs("EPSG:25832", "EPSG:4326", None).unwrap();

        loop {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Eof) => break,
                Err(e) => error!("Error at position {}: {:?}", reader.buffer_position(), e),

                Ok(Event::Start(ref e)) if e.name().as_ref() == b"stopFacility" => {
                    let mut id = String::new();
                    let mut x = 0.0;
                    let mut y = 0.0;
                    let mut name = None;
                    let mut stop_area_id = None;

                    for attr in e.attributes().flatten() {
                        match attr.key.as_ref() {
                            b"id" => id = attr.unescape_value().unwrap().to_string(),
                            b"x" => {
                                x = attr
                                    .unescape_value()
                                    .unwrap()
                                    .to_string()
                                    .parse()
                                    .unwrap_or(0.0)
                            }
                            b"y" => {
                                y = attr
                                    .unescape_value()
                                    .unwrap()
                                    .to_string()
                                    .parse()
                                    .unwrap_or(0.0)
                            }
                            b"name" => name = Some(attr.unescape_value().unwrap().to_string()),
                            b"stopAreaId" => {
                                stop_area_id = Some(attr.unescape_value().unwrap().to_string())
                            }
                            _ => (),
                        }
                    }

                    let (lon, lat) = proj.convert((x, y)).unwrap_or((0.0, 0.0));

                    let stop_facility = StopFacility {
                        id: id.clone(),
                        x,
                        y,
                        name: name.clone(),
                        stop_area_id,
                        lat: Some(lat),
                        lon: Some(lon),
                        routes: Vec::new(),
                        lines: Vec::new(),
                        departures: Vec::new(),
                    };
                    let entity = world.spawn(stop_facility).id();
                    {
                        let mut schedule = world.get_resource_mut::<Schedule>().unwrap();
                        schedule.stop_facilities.insert(id.clone(), entity);
                    }
                    debug!(
                        "Added stop facility {} with id: {}",
                        name.unwrap_or_default(),
                        id
                    );
                }
                Ok(Event::Start(ref e)) if e.name().as_ref() == b"transitLine" => {
                    debug!("Parsing transitLine element: {:?}", e);
                    for attr in e.attributes().flatten() {
                        if attr.key.as_ref() == b"id" {
                            current_line = Some(attr.unescape_value().unwrap().to_string());
                        }
                        if attr.key.as_ref() == b"name" {
                            current_line_name = Some(attr.unescape_value().unwrap().to_string());
                        }
                    }
                    in_line = true;
                    after_line_attributes = false;
                }
                Ok(Event::Start(ref e))
                    if in_line && !after_line_attributes && e.name().as_ref() == b"attributes" =>
                {
                    debug!("Parsing attributes for transitLine: {:?}", e);
                    in_line_attributes = true;
                }
                Ok(Event::Start(ref e))
                    if in_line && in_line_attributes && e.name().as_ref() == b"attribute" =>
                {
                    debug!("Parsing attribute element for transitLine: {:?}", e);
                    for attr in e.attributes().flatten() {
                        if attr.key.as_ref() == b"name" {
                            current_attribute_name =
                                Some(attr.unescape_value().unwrap().to_string());
                        }
                    }
                }
                Ok(Event::Text(e)) if in_line && in_line_attributes => {
                    let text = e.decode().unwrap().to_string();
                    if let Some(attr_name) = &current_attribute_name {
                        debug!("Parsing text for attribute element {}: {}", attr_name, text);
                        match attr_name.as_str() {
                            "gtfs_agency_id" => current_line_agency_id = Some(text),
                            "gtfs_route_type" => {
                                if let Ok(value) = text.parse::<u16>() {
                                    current_line_route_type = RouteType::from_int(value);
                                }
                            }
                            _ => (),
                        }
                    }
                }
                Ok(Event::End(ref e))
                    if in_line && in_line_attributes && e.name().as_ref() == b"attribute" =>
                {
                    debug!(
                        "Finished parsing attribute element for transitLine: {:?}",
                        e
                    );
                    current_attribute_name = None;
                }
                Ok(Event::End(ref e))
                    if in_line && in_line_attributes && e.name().as_ref() == b"attributes" =>
                {
                    debug!("Finished parsing attributes for transitLine: {:?}", e);
                    in_line_attributes = false;
                    after_line_attributes = true;
                    let line = Line {
                        id: current_line.clone().unwrap_or_default(),
                        name: current_line_name.clone().unwrap_or_default(),
                        agency_id: current_line_agency_id.clone(),
                        route_type: current_line_route_type.clone(),
                        routes: Vec::new(),
                        trips: Vec::new(),
                    };
                    let entity = world.spawn(line.clone()).id();
                    {
                        let mut schedule = world.get_resource_mut::<Schedule>().unwrap();
                        schedule
                            .lines
                            .insert(current_line.clone().unwrap_or_default(), entity);
                    }
                    current_line_entity = Some(entity);
                    debug!("Created line entity: {:?}", line);
                }
                Ok(Event::Start(ref e)) if in_line && e.name().as_ref() == b"transitRoute" => {
                    debug!("Parsing transitRoute element: {:?}", e);
                    for attr in e.attributes().flatten() {
                        if attr.key.as_ref() == b"id" {
                            current_route = Some(attr.unescape_value().unwrap().to_string());
                        }
                    }
                    in_route = true;
                    if let Some(line_entity) = current_line_entity {
                        let route = Route {
                            id: current_route.clone().unwrap_or_default(),
                            route_profile: Vec::new(),
                            departures: Vec::new(),
                            trips: Vec::new(),
                            line: line_entity,
                        };
                        let entity = world.spawn(route.clone()).id();
                        {
                            let mut schedule = world.get_resource_mut::<Schedule>().unwrap();
                            schedule.routes.push(entity);
                        }
                        current_route_entity = Some(entity);
                        // Also add to line's routes
                        if let Some(mut line) = world.get_mut::<Line>(line_entity) {
                            line.routes.push(entity);
                        }
                        debug!("Created route entity: {:?}", route);
                    }
                }
                Ok(Event::Start(ref e)) if in_route && e.name().as_ref() == b"routeProfile" => {
                    debug!("started routeProfile element");
                    in_route_profile = true;
                }
                Ok(Event::Empty(ref e)) | Ok(Event::Start(ref e))
                    if in_route && in_route_profile && e.name().as_ref() == b"stop" =>
                {
                    debug!("Parsing stop element in routeProfile: {:?}", e);
                    let mut stop_id = None;
                    let mut arrival_offset = TimeDelta::zero();
                    let mut departure_offset = TimeDelta::zero();

                    for attr in e.attributes().flatten() {
                        match attr.key.as_ref() {
                            b"refId" => stop_id = Some(attr.unescape_value().unwrap().to_string()),
                            b"arrivalOffset" => {
                                if let Ok(offset) = attr.unescape_value() {
                                    arrival_offset =
                                        NaiveTime::parse_from_str(offset.as_ref(), "%H:%M:%S")
                                            .map(|t| {
                                                TimeDelta::hours(t.hour() as i64)
                                                    + TimeDelta::minutes(t.minute() as i64)
                                                    + TimeDelta::seconds(t.second() as i64)
                                            })
                                            .unwrap_or(TimeDelta::zero());
                                }
                            }
                            b"departureOffset" => {
                                if let Ok(offset) = attr.unescape_value() {
                                    departure_offset =
                                        NaiveTime::parse_from_str(offset.as_ref(), "%H:%M:%S")
                                            .map(|t| {
                                                TimeDelta::hours(t.hour() as i64)
                                                    + TimeDelta::minutes(t.minute() as i64)
                                                    + TimeDelta::seconds(t.second() as i64)
                                            })
                                            .unwrap_or(TimeDelta::zero());
                                }
                            }
                            _ => (),
                        }
                    }

                    if let Some(stop_id) = stop_id {
                        let stop_facilities;
                        {
                            let schedule = world.get_resource::<Schedule>().unwrap();
                            stop_facilities = schedule.stop_facilities.clone();
                        }
                        if let Some(&stop_entity) = stop_facilities.get(&stop_id)
                            && let Some(route_entity) = current_route_entity
                        {
                            let route_profile_point = RouteProfilePoint {
                                route: route_entity,
                                stop: stop_entity,
                                arrival_offset,
                                departure_offset,
                            };
                            let entity = world.spawn(route_profile_point.clone()).id();
                            if let Some(mut route) = world.get_mut::<Route>(route_entity) {
                                debug!(
                                    "Added route profile point for stop {} to route {:?}: {:?}",
                                    stop_id, route, route_profile_point
                                );
                                route.route_profile.push(entity);
                            }
                        }
                    }
                }
                Ok(Event::End(ref e))
                    if in_route && in_route_profile && e.name().as_ref() == b"routeProfile" =>
                {
                    debug!("Finished parsing routeProfile elements");
                    in_route_profile = false;
                }
                Ok(Event::Start(ref e)) if in_route && e.name().as_ref() == b"departures" => {
                    debug!("started departures element");
                    in_departures = true;
                }
                Ok(Event::Start(ref e)) | Ok(Event::Empty(ref e))
                    if in_route && in_departures && e.name().as_ref() == b"departure" =>
                {
                    debug!("Parsing departure element in departures: {:?}", e);
                    let mut departure_time = None;
                    let mut departure_id = None;
                    for attr in e.attributes().flatten() {
                        match attr.key.as_ref() {
                            b"id" => {
                                departure_id = Some(attr.unescape_value().unwrap().to_string())
                            }
                            b"departureTime" => {
                                if let Ok(time_str) = attr.unescape_value()
                                    && let Ok(naive_time) =
                                        NaiveTime::parse_from_str(time_str.as_ref(), "%H:%M:%S")
                                {
                                    let today = Utc::now().date_naive();
                                    departure_time = Some(today.and_time(naive_time));
                                }
                            }
                            _ => (),
                        }
                    }
                    if let (Some(dep_time), Some(dep_id)) = (departure_time, departure_id)
                        && let Some(route_entity) = current_route_entity
                    {
                        let departure = RouteDeparture {
                            id: dep_id,
                            departure_time: dep_time,
                            route: route_entity,
                            trip: None,
                        };
                        let entity = world.spawn(departure).id();
                        {
                            let mut schedule = world.get_resource_mut::<Schedule>().unwrap();
                            schedule.departures.push(entity);
                        }
                        if let Some(mut route) = world.get_mut::<Route>(route_entity) {
                            route.departures.push(entity);
                        }
                    }
                }
                Ok(Event::End(ref e)) if in_route && e.name().as_ref() == b"departures" => {
                    debug!("Finished parsing departures elements");
                    in_departures = false;
                }
                Ok(Event::End(ref e)) if in_route && e.name().as_ref() == b"transitRoute" => {
                    debug!("Finished parsing transitRoute element");
                    current_route = None;
                    current_route_entity = None;
                    in_route = false;
                }
                Ok(Event::End(ref e)) if e.name().as_ref() == b"transitLine" => {
                    debug!("Finished parsing transitLine element");
                    current_line = None;
                    current_line_name = None;
                    current_line_agency_id = None;
                    current_line_route_type = None;
                    in_line = false;
                }
                _ => (),
            };
            buf.clear();
        }
    }

    pub fn create_trips_and_departures(world: &mut World) {
        // For each route, create trips and departures

        let routes = world.get_resource::<Schedule>().unwrap().routes.clone();
        for &route_entity in &routes {
            let (route_profile_entities, departure_entities, route_id, line) = {
                if let Some(route) = world.get::<Route>(route_entity) {
                    (
                        route.route_profile.clone(),
                        route.departures.clone(),
                        route.id.clone(),
                        route.line,
                    )
                } else {
                    continue;
                }
            };

            let mut route_profile_points: Vec<RouteProfilePoint> = route_profile_entities
                .iter()
                .filter_map(|&e| world.get::<RouteProfilePoint>(e).cloned())
                .collect();
            route_profile_points.sort_by_key(|p| p.departure_offset);

            for &departure_entity in &departure_entities {
                let first_departure: RouteDeparture;
                if let Some(dep) = world.get::<RouteDeparture>(departure_entity) {
                    first_departure = dep.clone();
                } else {
                    continue;
                };

                let trip_id = format!("trip_{}_{}", route_id.clone(), first_departure.id);
                let headsign = route_profile_points.last().and_then(|p| {
                    if let Some(stop) = world.get::<StopFacility>(p.stop) {
                        stop.name.clone()
                    } else {
                        None
                    }
                });
                let trip = Trip {
                    id: trip_id.clone(),
                    route: route_entity,
                    line,
                    departures: Vec::new(),
                    headsign: headsign.clone(),
                };
                let trip_entity = world.spawn(trip).id();
                {
                    let mut schedule = world.get_resource_mut::<Schedule>().unwrap();
                    schedule.trips.push(trip_entity);
                }
                // Link trip to route
                if let Some(mut route) = world.get_mut::<Route>(route_entity) {
                    route.trips.push(trip_entity);
                }
                let mut line_name = String::new();
                if let Some(mut line) = world.get_mut::<Line>(line) {
                    line.trips.push(trip_entity);
                    line_name = line.name.clone();
                }
                // Link trip to line
                // Create departures for each stop in the route profile
                for profile_point in &route_profile_points {
                    let departure_time =
                        first_departure.departure_time + profile_point.departure_offset;
                    let departure = Departure {
                        line_name: line_name.clone(),
                        route_headsign: headsign.clone().unwrap_or_default(),
                        arrival_time: Some(
                            first_departure.departure_time + profile_point.arrival_offset,
                        ),
                        departure_time: Some(departure_time),
                        stop: profile_point.stop,
                        trip: trip_entity,
                        route: route_entity,
                    };
                    let departure_entity = world.spawn(departure).id();
                    {
                        let mut schedule = world.get_resource_mut::<Schedule>().unwrap();
                        schedule.departures.push(departure_entity);
                    }
                    // Link departure to stop facility
                    if let Some(mut stop_facility) =
                        world.get_mut::<StopFacility>(profile_point.stop)
                    {
                        stop_facility.departures.push(departure_entity);
                    }
                    // Link departure to trip
                    if let Some(mut trip_mut) = world.get_mut::<Trip>(trip_entity) {
                        trip_mut.departures.push(departure_entity);
                    }
                }
            }
        }
    }
}

impl Plugin for SchedulePlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Schedule::new(self.path.clone()))
            .add_systems(Startup, SchedulePlugin::load)
            .add_systems(
                Startup,
                SchedulePlugin::create_trips_and_departures.after(SchedulePlugin::load),
            );
    }
}
