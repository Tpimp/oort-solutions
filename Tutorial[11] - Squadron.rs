// Tutorial: Squadron
// Destroy the enemy ships. They now shoot back.
// Tutorial: Missiles
// Destroy the enemy ship with your missiles.
// Hint: https://en.wikipedia.org/wiki/Proportional_navigation
// use oort_api::prelude::*;


// Tutorial: Radio
// Destroy the enemy ship. Your radar is broken, but a radio signal on channel
// 2 will give you its position and velocity.
use oort_api::prelude::*;
const BULLET_SPEED: f64 = 1000.0; // m/s

/**************************************************************
* Tutorial 9: Radio
* Author: Christopher Dean
* Last Update: 11/03/23
* Adjusted to using the radio on channel 2 to supply position
* and velocity
* 25.717s on tutorial 9
****************************************************************/
const TICKS_PER_SECOND: f64 = 60.0;
const BULLET_SPEED_PER_TICK: f64 = BULLET_SPEED / TICKS_PER_SECOND;
const TICKS_PER_FIRE: u32 = 4;
const SEEK_AND_DESTROY: u64 = 1337;
const POSITIONING_CHANNEL: usize = 2;

/************************************************************
*  ** Hive Mind **
*
*
************************************************************/
static mut TIMES_CALLED: u32 = 0;
pub struct HiveMind {
    ship_count: u32,
    ship_ids: Vec<u32>
}

impl HiveMind {
    pub fn hive_mind_tick(&mut self) {

    }
    pub fn register_ship(&mut self, ship: &Ship, id: u32) {
        if ship.ship_class == Class::Fighter && !self.ship_ids.contains(&id){
            self.ship_count += 1;
            self.ship_ids.push(id);
        }
    }
    pub fn new() -> HiveMind {
        HiveMind {
            ship_count: 0,
            ship_ids: Vec::new()
        }
    }
}
static mut HIVE_MIND:HiveMind = HiveMind{ship_count:0, ship_ids: Vec::new()};


/*****************************************************
* Utility Structs
* CurveBehavior captures multiple data points related
* to the acceleration curve, most importantly throttle
* throttle is the calculated "next" torque value
*******************************************************/
pub struct CurveBehavior{
    valid: bool,
    decelerate: bool,
    ticks_to_accel:u32,
    throttle: f64    
}

enum RadarStates{
    SweepingState= 0,
    TargetUpdate = 1
}

pub struct RadarData{
    sweep_step: f64,
    state: RadarStates,
    target_last_position: Vec2,
    target_last_velocity: Vec2,
    target_last_rssi: f64,
    target_last_snr: f64
}

fn calculate_angular_velocity(tune_factor: f64, angle_to_mark: f64) -> f64 {
    let c1: f64 = 2.0 * tune_factor.sqrt();
    tune_factor * angle_to_mark - c1 * angular_velocity()
}

pub struct Ship {
    use_burst_fire: bool,
    target_heading :  Option<f64>,
    target_position: Option<Vec2>,
    target_lead_position: Option<Vec2>,
    target_velocity: Option<Vec2>,
    target_acceleration: Option<Vec2>,
    next_torque: f64,
    objective: u64,
    state: String,
    should_fire_gun0: bool,
    trigger_tick: u32,
    gun0_fire_count: u32,
    gun0_burst_fire: u32,
    gun0_burst_pause: u32,
    scan_position: Option<Vec2>,
    scan_velocity: Option<Vec2>,
    radar_cache: Option<RadarData>,
    missle_contact_position: Option<Vec2>,
    missle_contact_velocity: Option<Vec2>,
    ship_class: Class
    
}

impl Ship {
    pub fn new() -> Ship {
        let new_ship = Ship {
            use_burst_fire: false,
            target_heading : None,
            target_position : None,
            target_lead_position: None,
            target_velocity: None,
            target_acceleration: None,
            next_torque: 0.0,
            objective: SEEK_AND_DESTROY,
            //objective: !SEEK_AND_DESTROY, // For Fun, uncomment and comment the above line
            state: String::from("starting"),
            should_fire_gun0: false,
            trigger_tick: 0,
            gun0_fire_count: 0,
            gun0_burst_fire: 3, // USE To configure burst fire count
            gun0_burst_pause: 1,
            scan_position: None,
            scan_velocity: None,
            radar_cache: None,
            missle_contact_position: None,
            missle_contact_velocity: None,
            ship_class: class()
        };

        unsafe {
            TIMES_CALLED += 1;
        }
        return new_ship;
    }
    /******************************************************************
    * ** Radio System **
    *
    *******************************************************************/
    pub fn receive_radio(&mut self) -> Option<(Vec2, Vec2)> {
        set_radio_channel(POSITIONING_CHANNEL);
        if let Some(msg) = receive() {
            return Some((vec2(msg[0], msg[1]),vec2(msg[2], msg[3])));
        }
        return None;
    }

    pub fn send_radio(&mut self) {
        set_radio_channel(POSITIONING_CHANNEL);
        if self.target_lead_position.is_some() && self.target_velocity.is_some() {
            let position = self.target_lead_position.unwrap();
            let velocity = self.target_velocity.unwrap();
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
                fire(0);
                //fire(1);
            }
        }
    }

    pub fn update_missle(&mut self) {
        //if let Some(contact) = scan() {
            // self.scan_position = Some(contact.position);
            // self.scan_velocity = Some(contact.velocity);
            // let dp = contact.position - position();
            // let dv = contact.velocity - velocity();  
        self.update_radar();
        // calculate heading from lead position
        let mut has_target = false;
        let mut target_position = vec2(0.0,0.0);
        let mut target_velocity = vec2(0.0,0.0);
        if self.scan_position.is_some() {
            target_position = self.scan_position.unwrap();
            target_velocity = self.scan_velocity.unwrap();
            has_target = true;       
        } else {            
            let result = self.receive_radio();
            if result.is_some() {
                (target_position, target_velocity) = result.unwrap();
                has_target = true;
            }
        }
        if has_target {
            let line_diff = target_position - position();
            let current_diff = angle_diff(heading(), line_diff.angle());
            self.target_heading = Some(line_diff.angle());
            debug!("Target Position: {}", target_position);
            debug!("Position: {} ", position());
            debug!("Distance from target: {}", line_diff);
            debug!("Target velocity: {}", line_diff);
            debug!("Distance from target (length): {}", line_diff.length());
            debug!("Velocity: {}", velocity());
            turn(current_diff);
            accelerate(line_diff + (target_velocity * 1.565  * ((line_diff/BULLET_SPEED) + 0.35)));            
            draw_line(target_position, position(), 0xff0000);
            if line_diff.length() <= 122.0  {
                explode();
            }
        } else {
            accelerate(vec2(1000.0,0.0));
        }
            //self.update_guns();


        // Update Angular Velocity
        // Update Planar Thrust Vectors
        
           
        // } else {
        //     self.missle_contact_position = None;
        //     self.missle_contact_velocity = None;
        // }
         
    }

/*******************************************************************
* ** Radar and Enemy Tracking **
* This code is responsible for updating the targets and next
* aim for the weapons system
********************************************************************/
    // Seek
    pub fn update_radar(&mut self) {
        let mut scans = 0;
        if self.scan_position.is_none() {
            set_radar_heading(radar_heading() + 0.0628);
            set_radar_max_distance(BULLET_SPEED * 10.0);
            self.seek();
            scans += 1;
        }else {
            if self.target_heading.is_some() {
                set_radar_heading( (target() - position()).angle() );
                if self.target_lead_position.is_some() {
                   // set_radar_max_distance((position() - self.target_lead_position.unwrap()).length() * 1.5)
                }
                self.seek();
            }
        }
        //self.seek();
    }

    pub fn seek(&mut self) {        
        let scanned = scan();
        if scanned.is_some() {
            let result = scanned.unwrap();
            if result.class == Class::Fighter {
                self.scan_position = Some(result.position);
                self.scan_velocity = Some(result.velocity);
            }
        } else {
            self.scan_position = None;
            self.scan_velocity = None;
        }
        // if self.scan_result.is_some() {
        //     return self.scan_result;
        // }
        // return None;        
    }

    // Track
    pub fn track(&mut self, target: Vec2, target_velocity: Vec2, velocity: Vec2) -> Option<Vec2> {
        if self.target_heading.is_some() {
            self.should_fire_gun0 = angle_diff(heading(), self.target_heading.unwrap()).abs() < 0.018;        
        }else {
            self.should_fire_gun0 = false;
        }
        if target.x != 0.0 && target.y != 0.0 {
            let length_meters = (target - position()).length() as f64;
            let distance_ratio = (length_meters / BULLET_SPEED);
            // account for acceleration
            let mut target_acceleration = vec2(0.0, 0.0);
            if self.target_velocity.is_some() {
                target_acceleration = ((target_velocity - self.target_velocity.unwrap()) * TICKS_PER_SECOND)/2.0;
                self.target_acceleration = Some(target_acceleration);
            }
            // let mut jitter = 1.0;
            // match distance_ratio.abs() {
            //      4.001.. =>  {jitter = rand(0.875, 1.245);}
            //      2.65.. => {jitter = rand(0.99854, 1.00146);}
            //      1.45.. => {jitter = rand(0.999985, 1.000015);}
            //     _ => {self.use_burst_fire = false;}
            // }
            // if distance_ratio.abs() >= 2.0 {
            //     self.should_fire_gun0 = false;
            // } else if 
            //     jitter = rand(0.99954, 1.00046);
            // }
            // if distance_ratio.abs() > 1.55{
            //     jitter = rand(0.99999985, 1.00000015);
            //     //self.use_burst_fire = true;
            // } else {
            //     self.use_burst_fire = false;
            //}
            let next_target = target + (target_velocity - velocity) * distance_ratio.abs() + (target_acceleration * distance_ratio.abs());
            self.target_velocity = Some(target_velocity);
            return Some(next_target);
        }
        return None;
    }
/***********************************************************************/

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
    pub fn calculate_ticks_to_end_approach(&mut self) -> i64 {
        return 0;
    }
    pub fn update_engine_vectors(&mut self) {        
        if self.target_heading.is_some() && self.target_lead_position.is_some() { // Calculate the fastest rotation curve current heading, target_heading
            let current_diff = angle_diff(heading(), (self.target_lead_position.unwrap() - position()).angle());
            if current_diff.abs() > 0.205 {
                self.next_torque = calculate_angular_velocity(55.69 * current_diff.abs(), current_diff);                
            } else {// using my turning solution
                let acceleration_curve = self.find_highest_angular_curve(angular_velocity(), angle_diff(heading(),
                                                                        self.target_heading.unwrap()));
                self.next_torque = acceleration_curve.throttle;
            }
        } else {
            self.next_torque = 0.0;
        }

        // Update Angular Velocity
        torque(self.next_torque);
        // Update Planar Thrust Vectors
        if self.target_position.is_some() {
            accelerate(self.target_position.unwrap() + self.target_velocity.unwrap());            
        }else {
            accelerate(vec2(0.0, 0.0));
        }
    }


    pub fn update_normal_ship(&mut self) {
        self.update_radar();
        self.send_radio();
        if self.objective == SEEK_AND_DESTROY { // find and kill
            if self.scan_position.is_some() {
                let target_position = self.scan_position.unwrap();
                let target_velocity = self.scan_velocity.unwrap();
                self.target_lead_position = self.track(target_position, target_velocity, velocity());  // update aim and tracking position
            } else {
                self.target_lead_position = None;
            }
            if self.target_lead_position.is_some() {
                self.target_position = Some(self.approach_and_orbit(550.0, 1050.0, position(), self.target_lead_position.unwrap(), target_velocity())); // 300 meter orbit
            }
        } else { // locate next target

        }
        // calculate heading from lead position
        if self.target_lead_position.is_some() {
            self.target_heading = Some ((self.target_lead_position.unwrap() - position()).angle());
        } else {            
            self.target_heading = None;
        }
        self.update_guns();
        self.update_engine_vectors();
        self.draw_diagnostics();
    }

/********************************************************************************************************
* ** Mission Specific functions **
* Functions used to update the systems diagnostics
*********************************************************************************************************/
    pub fn tick(&mut self) {
        // higher level mission logic
        // if target, track
        // mission ends, always a target
        // track target

        unsafe{
            HIVE_MIND.register_ship(self, id());
        }
        set_radar_max_distance(world_size());
        if class() == Class::Missile {
             self.update_missle();
        }else {
            if(id() == 1) {
                let current_ticks = current_tick();
                if(current_ticks < 600) {
                    accelerate(vec2(10.0, 0.0));                    
                    fire(0);
                    fire(1);
                } else if current_ticks > 600 && current_ticks < 750 {             
                    accelerate(vec2(25.0, 0.0));                    
                    fire(0);
                    fire(1);
                } else {
                    self.update_normal_ship();
                }
            } else {
                if current_tick() < 1200 {
                    fire(0);
                    fire(1);
                    accelerate(vec2(50.0, 0.0));
                    //torque(rand(-0.02, 0.02));
                }else {
                    self.update_normal_ship();
                }
            }
        }
        // else {
        //    
        // }
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
        unsafe {
            debug!("Times Called {}", TIMES_CALLED);
        }
        unsafe {
            debug!("Hivemind controlling {}", HIVE_MIND.ship_count);
            debug!("Ship ID count {}", HIVE_MIND.ship_ids.len());
        }
        if self.target_heading.is_some() {
            debug!("Target Heading {}", self.target_heading.unwrap());
        }
        draw_line(position(), target(), 0x00ff00);
        if self.target_lead_position.is_some() {
            let lead_position = self.target_lead_position.unwrap();
            let dp = (lead_position - position());
            debug!("distance to target: {} meters", dp.length());
            debug!("time to target: {} seconds", dp.length() / BULLET_SPEED);
            if self.target_acceleration.is_some() {
                debug!("Target Acceleration: {} meters/s", self.target_acceleration.unwrap());
            }
            draw_line(position(), lead_position, 0xff0000);
        }
        if self.missle_contact_position.is_some() {
            draw_line(position(), self.missle_contact_position.unwrap(), 0x00aaff);
            debug!("Missle has contact: {}", self.missle_contact_position.unwrap());
            debug!("Missle scanned velocity: {}", self.missle_contact_velocity.unwrap());
        }
    }
/*******************************************************************************************************/
}