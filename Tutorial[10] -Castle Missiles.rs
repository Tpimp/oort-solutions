// Tutorial: Missiles
// Destroy the enemy ship with your missiles.
// Hint: https://en.wikipedia.org/wiki/Proportional_navigation

/**************************************************************
* Tutorial 10: Radio
* Author: Christopher Dean
* Last Update: 11/08/23
* Adjusted to using the radio on channel 2 to supply position
* and velocity
* Completely rebuilt organization with multiple ship
* sub types
* 25.717s on tutorial 10
****************************************************************/
const BULLET_SPEED: f64 = 1000.0; // m/s
const MISSILE_SPEED: f64 = 850.0;
const TICKS_PER_SECOND: f64 = 60.0;
const BULLET_SPEED_PER_TICK: f64 = BULLET_SPEED / TICKS_PER_SECOND;
const TICKS_PER_FIRE: u32 = 4;
const SEEK_AND_DESTROY: u64 = 1337;
const MISSILE_OUT: usize = 2;
const MISSILE_RELOAD_TIME:u32 = 120;
const RADIO_COUNT:usize = 4; // 4 radios for frigate, 8 for a cruiser
const MISSILE_RADIO:usize = RADIO_COUNT-1; // last radio
const POSITIONING_CHANNEL:usize = 2;
enum MessageID {

}

use oort_api::prelude::*;
use std::collections::VecDeque;

pub struct CurveBehavior{
    valid: bool,
    decelerate: bool,
    ticks_to_accel:u32,
    throttle: f64    
}

pub struct UnitDescription {
    class: Class,
    position: Vec2,
    velocity: Vec2,
    target_heading: f64,
    distance: f64, // precalculated, index
    lead_position: Option<Vec2>,
}

enum RadarState {
    InitState = 0,
    BroadScan = 1,
    NarrowScan = 2,
    POIScan = 3,
    NoneState
}

pub struct RadarData {
    last_state_heading: Option<f64>,
    next_heading: Option<f64>,
    radar_state: RadarState,
    previous_state: RadarState,
    tick_counter: u64
}

impl RadarData {
    pub fn create() -> RadarData {
        RadarData {
            last_state_heading: None,
            radar_state: RadarState::InitState,
            previous_state: RadarState::NoneState, 
            next_heading: None,
            tick_counter: 0
        }
    }
}

pub struct ZCruiser {
}


pub struct SharedData {

}

impl SharedData {
        pub fn create() -> SharedData {
        SharedData{
            
        }
    }
}

pub fn configure_cruiser() -> Ship {
    Ship {
        cruiser_data: Some (
            ZCruiser {

            }
        ),
        frigate_data: None,
        fighter_data: None,
        missle_data: None,
        shared_data: SharedData::create()
    }
}

pub struct TyFighter {
    radar: RadarData,
    target: Option<UnitDescription>,
    should_fire_gun0: bool,
    use_burst_fire: bool,
    gun0_burst_fire: u32,
    gun0_fire_count: u32,
    gun0_burst_pause: u32,
    target_acceleration: Option<Vec2>,
    acceleration: Vec2

}

impl TyFighter {
    pub fn configure_fighter() -> Ship {
        Ship {
            cruiser_data: None,
            frigate_data: None,
            fighter_data: Some (
                TyFighter {
                    radar: RadarData::create(),
                    target: None,
                    should_fire_gun0: false,
                    use_burst_fire: false,
                    gun0_burst_fire: 16,
                    gun0_fire_count: 0,
                    gun0_burst_pause: 2,
                    target_acceleration: None,
                    acceleration: vec2(0.0,0.0)
                }
            ),
            missle_data: None,
            shared_data: SharedData::create()
        }
    }

    /******************************************************************
    * ** Radio System **
    *
    *******************************************************************/
    pub fn receive_radio_target(&mut self) -> Option<(Vec2, Vec2)> {
        set_radio_channel(POSITIONING_CHANNEL);
        if let Some(msg) = receive() {
            return Some((vec2(msg[0], msg[1]),vec2(msg[2], msg[3])));
        }
        return None;
    }

    pub fn send_radio(&mut self) {
        set_radio_channel(POSITIONING_CHANNEL);
        if let Some(target) = self.target.as_ref() {
            let velocity = target.velocity;
            let mut position = vec2(0.0, 0.0);
            if let Some(lead_position) = target.lead_position {
                position = lead_position;
            } else {
                position = target.position;
            }
            send([position.x, position.y, velocity.x, velocity.y]);
        }
    }

    /*******************************************************************
    * ** Weapon Systems **
    * This code is responsible for aiming turrets, and firing weapons
    * using configured parameters
    ********************************************************************/
    pub fn start_firing(&mut self, burst_count: u32) {
        self.should_fire_gun0 = true;
        self.gun0_burst_fire = burst_count;
    }

    pub fn fire_burst(&mut self) {
        if self.gun0_fire_count / TICKS_PER_FIRE >= (self.gun0_burst_fire + self.gun0_burst_pause) {
            self.gun0_fire_count = 0;
            self.should_fire_gun0 = false;
        } else {
            if self.gun0_fire_count / TICKS_PER_FIRE < self.gun0_burst_fire  {
                fire(1);
            }
            self.gun0_fire_count += 1;
        }
    }
    pub fn update_guns(&mut self) {
        if self.should_fire_gun0 == true {
            if self.use_burst_fire {
                self.fire_burst();
            } else {
                fire(1);
            }
        }
    }

    pub fn find_target(&mut self) {
        let scanned = scan();
        if scanned.is_some() {
            let result = scanned.unwrap();
            let next_position = (result.position + result.velocity);
            let position_diff = next_position - oort_api::prelude::position();
            let distance = position_diff.length();
            let target_heading = position_diff.angle();
            let lead_position = self.track(result.position, result.velocity, velocity());
            self.target = Some(
                UnitDescription{
                    class: result.class,
                    position: result.position,
                    velocity: result.velocity,
                    target_heading,
                    distance,
                    lead_position
                }
            );
        } else {
            self.target = None;
        }
    }
    pub fn update_radar(&mut self) {
        let mut scans = 0;
        if self.target.is_none() {
            set_radar_heading(radar_heading() + 0.0628);
            set_radar_max_distance(BULLET_SPEED * 10.0);
            self.find_target();
            scans += 1;
        }else {
            let target = self.target.as_ref().unwrap();
            set_radar_heading( (target.position - position()).angle() );
            self.find_target();
        }
    }
    pub fn track(&mut self, target: Vec2, target_velocity: Vec2, velocity: Vec2) -> Option<Vec2> {
        if let Some(stored_target) = self.target.as_mut() {
            self.should_fire_gun0 = angle_diff(heading(), stored_target.target_heading).abs() < 0.0192; 
            let length_meters = (target - position()).length() as f64;
            let distance_ratio = (length_meters / MISSILE_SPEED);     
            let mut target_acceleration = vec2(0.0, 0.0);
                target_acceleration = ((target_velocity - stored_target.velocity) * TICKS_PER_SECOND)/2.0;
                self.target_acceleration = Some(target_acceleration);

            let next_target = target + (target_velocity - velocity) * distance_ratio.abs();
            stored_target.velocity = target_velocity;
            stored_target.lead_position = Some(next_target);
            return Some(next_target);
        }else {
            self.should_fire_gun0 = false;
        }
        return None;
    }


    /********************************************************************************************************
    * ** Diagnostics **
    * Functions used to update the systems diagnostics
    *********************************************************************************************************/
        pub fn draw_diagnostics(&mut self) {
            
            //debug!("Current State: {}", self.state);
            debug!("Angular Velocity: {}", angular_velocity());
            debug!("Ships Heading {}", heading());
            debug!("Ships Velocity {}", velocity());
            if let Some(target) = self.target.as_ref() {
                debug!("Target Heading {}", target.target_heading);
                let lead_position = target.lead_position.unwrap();
                let dp = (lead_position - position());
                debug!("distance to target: {} meters", dp.length());
                debug!("time to target: {} seconds", dp.length() / MISSILE_SPEED);
                if self.target_acceleration.is_some() {
                    debug!("Target Acceleration: {} meters/s", self.target_acceleration.unwrap());
                }
                draw_line(position(), lead_position, 0xff0000);
            }
            draw_line(position(), target(), 0x00ff00);
        }
    /*******************************************************************************************************/
    /*******************************************************************************************************************
    * ** Navigation System ** 
    * Handles navigating and calculating the next thruster vectors based on the target_position
    * The navigation system also helps to steer heading to target_heading
    ********************************************************************************************************************/
    pub fn approach_and_orbit(&mut self, orbit_min_distance: f64, orbit_max_distance: f64, position: Vec2, target_position: Vec2, target_velocity: Vec2) -> Vec2 {
        let distance = target_position - position;
        if orbit_max_distance < distance.length() { // approach
            let seconds_apart = target_position / velocity();
            if seconds_apart.x > 10.0 || seconds_apart.y > 10.0 {
                return distance + target_velocity;
            } else {
                return (distance/8.0) + target_velocity;
            }
        }
        if orbit_min_distance > distance.length() {
           return (-0.65 * velocity() ) + target_velocity;
        }else {
           return (-1.9 * velocity()) + target_velocity;
        }
        return vec2(0.0, 0.0);
    }
    /********************************************************************************************************************
    * ** Engine Thrust and Drive System **
    * This code is responsible for updating the ships next torque and accelerate values
    *******************************************************************************************************************/
    fn calculate_angular_velocity(tune_factor: f64, angle_to_mark: f64) -> f64 {
        let c1: f64 = 2.0 * tune_factor.sqrt();
        tune_factor * angle_to_mark - c1 * angular_velocity()
    }
    // Calculates the smoothest and quickest stop torque value
    pub fn get_stop_torque(&mut self ) -> f64 {
        let mut ret_torque = 0.0;
        if angular_velocity().abs() > 0.001 { // finite stop
            if angular_velocity().abs() > max_angular_acceleration() {
                if angular_velocity() > 0.001 {
                    ret_torque = -max_angular_acceleration();
                } else if angular_velocity() < 0.001 {
                    ret_torque = max_angular_acceleration();
                } else {}
            } else { // dither stop
                if(angular_velocity().abs() < 0.1) {
                    let mut opposite_torque = angular_velocity() * 10.0;
                    if opposite_torque.abs() < 0.1 {
                        opposite_torque = 0.001; // when angular_velocity is real small
                    }
                    if angular_velocity() > 0.001 {
                        ret_torque = -opposite_torque;
                    } else if angular_velocity() < 0.001 {
                        ret_torque = opposite_torque;
                    }  else {}
                } else {
                    if angular_velocity() > 0.001 {
                        ret_torque = -max_angular_acceleration();
                    } else if angular_velocity() < 0.001 {
                        ret_torque = max_angular_acceleration();
                    } else {}
                }
            }
        } else {} // else, set torque to 0
        return ret_torque;
    }



    // low level "turn(angle)" replacement, rotates as quickly as possible
    pub fn find_highest_angular_curve(&mut self, start_velocity: f64, rotation_angle: f64) -> CurveBehavior {
        let mut curve = CurveBehavior{ valid:false, decelerate: false, ticks_to_accel: 0, throttle:0.0};
        let ticks_to_stop = start_velocity.abs() / max_angular_acceleration();
        // TODO: Get back to calculating if continuing in the same direction is faster
        // if (rotation_angle < 0.0) ^ (start_velocity < 0.0) { // Will potentially need to stop and turn back
        //     curve.ticks_to_stop = (start_velocity.abs() / max_angular_acceleration()).trunc() as u32;

        // } else {
            // already accelerating the correct direction
        let ticks_remaining = rotation_angle.abs()/angular_velocity().abs();
        if ticks_remaining > ticks_to_stop { // accelerate
            let mut ticks_to_accelerate = 0.0;
            let mut not_done = true;
            while ticks_to_accelerate < 2.0 && not_done {
                let next_speed = max_angular_acceleration() * ticks_to_accelerate;
                let new_ticks_to_stop = next_speed.abs() / max_angular_acceleration();
                let ticks_left = rotation_angle.abs() / next_speed;
                if new_ticks_to_stop > ticks_left {
                    not_done = false;
                }else {
                    ticks_to_accelerate += 1.0;
                }                
            }
            if ticks_to_accelerate > 2.0 {
                curve.throttle = max_angular_acceleration();
            } else {
                curve.throttle = (ticks_to_accelerate  * max_angular_acceleration()) / 2.0;
            }
            if rotation_angle.is_negative() {
                curve.throttle *= -1.0;
            }
            curve.ticks_to_accel = (ticks_to_accelerate / 2.0) as u32;
            curve.decelerate = false;
        }else { // slowing
            // calculate stop
            curve.throttle = self.get_stop_torque();
            curve.decelerate = true;
        }
        // }
        return curve;
    }

    pub fn update_engine_vectors(&mut self) {
        let mut torque_val = 0.0;        
        if self.target.is_some() { // Calculate the fastest rotation curve current heading, target_heading
            let target = self.target.as_ref().unwrap();
            let current_diff = angle_diff(heading(), (target.lead_position.unwrap() - position()).angle());
            if current_diff.abs() > 0.115 {
                torque_val = Self::calculate_angular_velocity(55.69 * current_diff.abs(), current_diff);                
            } else {// using my turning solution
                let acceleration_curve = self.find_highest_angular_curve(angular_velocity(), angle_diff(heading(),
                                                                        target.target_heading));
                torque_val = acceleration_curve.throttle;
            }
        } else {
            torque_val = 0.0;
        }

        // Update Angular Velocity
        torque(torque_val);
        // Update Planar Thrust Vectors
        let mut velocity = vec2(0.0, 0.0);
        if let Some(target_ref) = self.target.as_ref() {
            velocity = target_ref.velocity;
        }
        accelerate(self.acceleration + velocity);            
    }


    /********************************************************************************************************
    * ** Mission Specific functions **
    * Functions used to update the systems diagnostics
    *********************************************************************************************************/
    pub fn tick(&mut self, _shared:&mut SharedData) {
        self.update_radar();
        self.send_radio();
        let mut t_velocity = vec2(0.0,0.0);
        let mut t_position = vec2(0.0,0.0);
        if let Some(target) = self.target.as_mut() {
            t_velocity = target.velocity;
            t_position = target.position;            
            if let Some(lead_position) = target.lead_position {
                self.acceleration = self.approach_and_orbit(550.0, 1050.0, position(), lead_position, t_velocity); 
            } else {
                self.acceleration = vec2(0.0,0.0);
            }
        }
        self.track(t_position, t_velocity, velocity());  // update aim and tracking position
        self.update_guns();
        self.update_engine_vectors();
        self.draw_diagnostics();
    }
}



/******************************************************************************************
* ** Missile Type ** 
*
*
******************************************************************************************/
pub struct XMissle {
    should_blow: bool,
    target: Option<UnitDescription>
}

impl XMissle {
    // Initialize Ship->XMissle
    pub fn configure_missle() -> Ship {        
        set_radio_channel(MISSILE_OUT);
        set_radar_width(PI/16.0);
        set_radar_heading(heading());
        Ship {
            cruiser_data: None,
            frigate_data: None,
            fighter_data: None,
            missle_data: Some (
                XMissle {
                    should_blow: false,
                    target: None,
                }
            ),
            shared_data: SharedData::create()
        }
    }
    pub fn track(&mut self, target: Vec2, target_velocity: Vec2, velocity: Vec2) -> Option<Vec2> {
        if let Some(stored_target) = self.target.as_mut() {
            let length_meters = (target - position()).length() as f64;
            let distance_ratio = (length_meters / MISSILE_SPEED);     
            let mut target_acceleration = vec2(0.0, 0.0);
            target_acceleration = ((target_velocity - stored_target.velocity) * TICKS_PER_SECOND)/2.0;
            let next_target = target + (target_velocity - velocity) * distance_ratio.abs();
            stored_target.velocity = target_velocity;
            return Some(next_target);          
        }
        return None;
    }
    

    pub fn check_for_target(&mut self) {
        // if no target, it has not even received the ships 'initial' target yet, check radio          
        if let Some(msg) = receive() { // check radio
            // try to read the communicated target
            let position = vec2(msg[0],msg[1]);
            let velocity = vec2(msg[2],msg[3]);
            let next_position = (position + velocity);
            let position_diff = next_position -  oort_api::prelude::position();
            let distance = position_diff.length();
            let target_heading = position_diff.angle();
            let lead_position = self.track(position, velocity,  oort_api::prelude::velocity());
            self.target = Some(
                UnitDescription{
                    class: Class::Unknown,
                    position,
                    velocity,
                    target_heading,
                    distance,
                    lead_position

                }
            );
        } else {
            self.target = None;
        }
    }
    pub fn update_targets(&mut self)  {
        if let Some(scanned_target) = scan() {
            // check if the target is a scan of an existing target
            // do calcs
            let next_position = (scanned_target.position + scanned_target.velocity);
            let position_diff = next_position -  oort_api::prelude::position();
            let distance = position_diff.length();
            let target_heading = position_diff.angle();
            let lead_position = self.track(scanned_target.position, scanned_target.velocity,  oort_api::prelude::velocity());
            self.target = Some(
                UnitDescription{
                    class: scanned_target.class,
                    position: scanned_target.position,
                    velocity: scanned_target.velocity,
                    target_heading,
                    distance,
                    lead_position
            });         
        } else {
            self.target = None;
        }
    }
    // Radar Targets and POI Acquisition
    pub fn update_radar(&mut self, _shared:&mut SharedData) {        
        self.update_targets();
        if self.target.is_some() {
            // unwrap get target by reference
            let target_data = self.target.as_ref().unwrap();
            set_radar_heading( (target_data.position - position()).angle());
        } else {
            set_radar_heading(radar_heading() + 0.125);
        }
        
        //self.seek();
    }
    // High level missile procedural logic
    pub fn tick(&mut self, _shared:&mut SharedData) {
        if self.target.is_none() { 
            self.check_for_target();
        }
        let mut has_target = false;
        let mut target_position = vec2(0.0,0.0);
        let mut target_velocity = vec2(0.0,0.0);
        let mut target_distance = 0.0;
        let mut target_heading = 0.0;
        let mut target_class = Class::Unknown;
        if(self.should_blow) { 
            explode();
        } else {
            if self.target.is_some() {
                let target_desc = self.target.as_ref().unwrap();
                target_position = target_desc.position;
                target_velocity = target_desc.velocity;
                target_heading = target_desc.target_heading;
                target_distance = target_desc.distance;
                target_class = target_desc.class;
                has_target = true;     
            }  
            if has_target {
                let line_diff = target_position - position();
                let current_diff = angle_diff(heading(), line_diff.angle());
                match target_class {
                    Class::Fighter =>{debug!("Target Class: {}", "Fighter");}
                    _ => {debug!("Target Class: {}", "Unknown"); }
                }
                let true_distance = 0.0;
                
                debug!("Target Position: {}", target_position);
                debug!("Distance from target: {}", line_diff.length());
                debug!("Target velocity: {}", target_velocity);
                debug!("Target Heading: {}", target_heading);
                debug!("Position: {} ", position());
                debug!("Velocity: {}", velocity());
                turn(current_diff * 1.09);
                accelerate((line_diff + (target_velocity * 2.255  * ((line_diff/MISSILE_SPEED) + 0.36))));            
                draw_line(target_position, position(), 0xff0000);
                if line_diff.length() <= 75.0 ||  fuel() == 0.0 {
                    self.should_blow = true;
                }
            }         
            debug!("Fuel {} ", fuel());
            self.update_radar(_shared);
        }
    }
    
}
// ******** END Missile Type ************************



/******************************************************************************************
* ** Frigate Type ** 
*
******************************************************************************************/
pub struct SupaFrigate {
    ticks_till_reload_missile: u32,
    radar: RadarData,
    targets: VecDeque<UnitDescription>,
    dodge_or_kill: VecDeque<UnitDescription>,
    target_lock: Option<UnitDescription>
    //friendly_units: Vec<UnitDescription>
}
impl SupaFrigate {  
    pub fn configure_frigate() -> Ship {        
        set_radio_channel(MISSILE_OUT);
        set_radar_width(0.0185);
        Ship {
            cruiser_data: None,
            frigate_data: Some (
                SupaFrigate {
                    ticks_till_reload_missile: 0,
                    radar: RadarData::create(),
                    targets: VecDeque::new(),
                    dodge_or_kill: VecDeque::new(),
                    target_lock: None
                }
            ),
            fighter_data: None,
            missle_data: None,
            shared_data: SharedData::create()
        }
    }
    pub fn draw_targets(&mut self) {
        debug!("Targets: {}", self.targets.len());
        for target in self.targets.iter_mut() {
            draw_square(target.position, 20.0, 0xff0000);
            draw_diamond(target.position + target.velocity, 20.0, 0x00ff00);
            draw_diamond(target.position + (target.velocity*2.0), 20.0, 0x00ff00);
            draw_diamond(target.position + (target.velocity*3.0), 20.0, 0x00ff00);
        }
    }

    pub fn update_targets(&mut self) {
    }
    pub fn update_radar(&mut self) {
    }
    pub fn send_target_to_missle(&mut self) {
    }
    pub fn scan_radio(&mut self) {        
    }   
    pub fn tick(&mut self, _shared:&mut SharedData) {           
    }
}


// Implementation of Ship -> aka ShipWrapper
pub struct Ship {
    cruiser_data: Option<ZCruiser>,    
    frigate_data: Option<SupaFrigate>,
    fighter_data: Option<TyFighter>,
    missle_data: Option<XMissle>,
    shared_data: SharedData
}

impl Ship {
    pub fn new() -> Ship {
        match class() { 
            Class::Frigate =>SupaFrigate::configure_frigate(),
            Class::Fighter =>TyFighter::configure_fighter(),
            Class::Cruiser =>configure_cruiser(),
            Class::Missile =>XMissle::configure_missle(),
            _ => {
                Ship {
                    cruiser_data: None,
                    frigate_data: None,
                    fighter_data: None,
                    missle_data: None,
                    shared_data: SharedData::create()
                } 
            }
        }
    }

    pub fn tick(&mut self) {
        if self.fighter_data.is_some() {
            self.fighter_data.as_mut().unwrap().tick(&mut self.shared_data);
        }
        if self.missle_data.is_some() {
            self.missle_data.as_mut().unwrap().tick(&mut self.shared_data);
        }

    }
}