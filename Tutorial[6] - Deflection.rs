use oort_api::prelude::*;
const BULLET_SPEED: f64 = 1000.0; // m/s

/**************************************************************
* Tutorial 6: Deflection
* Author: Christopher Dean
* Last Update: 10/30/23
* Improved fast and accurate turning, now approaching target
* 4.880s on Tutorial 6
****************************************************************/
const TICKS_PER_SECOND: f64 = 60.0;
const BULLET_SPEED_PER_TICK: f64 = BULLET_SPEED / TICKS_PER_SECOND;
const TICKS_TO_WAIT: f64 = 120.0;
const TICKS_TO_ACCEL: f64 =  TICKS_TO_WAIT + 36.0;
const TICKS_PER_FIRE: u32 = 4;
const SEEK_AND_DESTROY: u64 = 1337;

/*****************************************************
* Utility Structs
*
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

fn calculate_angular_velocity(tune_factor: f64, angle_to_mark: f64) -> f64 {
    let c1: f64 = 2.0 * tune_factor.sqrt();
    tune_factor * angle_to_mark - c1 * angular_velocity()
}

pub struct Ship {
    use_burst_fire: bool,
    target_heading :  Option<f64>,
    target_position: Option<Vec2>,
    target_lead_position: Option<Vec2>,
    next_torque: f64,
    counter: f64,
    objective: u64,
    state: String,
    should_fire_gun0: bool,
    trigger_tick: u32,
    gun0_fire_count: u32,
    gun0_burst_fire: u32
}

impl Ship {
    pub fn new() -> Ship {
        Ship {
            use_burst_fire: true,
            target_heading : None,
            target_position : None,
            target_lead_position: None,
            next_torque: 0.0,
            counter: 120.0,
            objective: SEEK_AND_DESTROY,
            //objective: !SEEK_AND_DESTROY, // For Fun, uncomment and comment the above line
            state: String::from("starting"),
            should_fire_gun0: false,
            trigger_tick: 0,
            gun0_fire_count: 0,
            gun0_burst_fire: 10 // USE To configure burst fire count
        }
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
    pub fn start_firing(&mut self, burst_count: u32) {
        self.should_fire_gun0 = true;
        self.gun0_burst_fire = burst_count;
    }

    pub fn fire_burst(&mut self) {
        if self.gun0_fire_count / TICKS_PER_FIRE >= (self.gun0_burst_fire + 1) {
            self.gun0_fire_count = 0;
            self.should_fire_gun0 = false;
        } else {
            if self.gun0_fire_count / TICKS_PER_FIRE < self.gun0_burst_fire  {
                fire(0);
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
            }
        }
    }

/*******************************************************************
* ** Radar and Enemy Tracking **
* This code is responsible for updating the targets and next
* aim for the weapons system
********************************************************************/
    // 
    pub fn track(&mut self, target: Vec2, target_velocity: Vec2, velocity: Vec2) -> Vec2 {
        let mut next_target = vec2(0.0,0.0);
            if self.target_heading.is_some() {
                self.should_fire_gun0 = angle_diff(heading(), self.target_heading.unwrap()).abs() < 0.018;        
            }else {
                self.should_fire_gun0 = false;
            }
        if target.x != 0.0 && target.y != 0.0 {
            let length_meters = (target - position()).length() as f64;
            let mut distance_ratio = (length_meters / BULLET_SPEED);            
            next_target = target + (target_velocity - velocity) * distance_ratio;
        }

        return next_target;
    }
/***********************************************************************/

/*******************************************************************************************************************
* ** Navigation System ** 
* Handles navigating and calculating the next thruster vectors based on the target_position
* The navigation system also helps to steer heading to target_heading
********************************************************************************************************************/
    pub fn approach_and_orbit(&mut self, orbit_min_distance: f64, orbit_max_distance: f64, position: Vec2, target_position: Vec2, target_velocity: Vec2) -> Vec2 {
        let distance = target_position - position;
        if orbit_min_distance < distance.length() {
           return 0.02 * (distance + target_velocity);
        }
        if orbit_max_distance > distance.length() {
           return -0.08 * (distance + target_velocity);
        }
        return vec2(0.0, 0.0);
    }
/********************************************************************************************************************
* ** Engine Thrust and Drive System **
* This code is responsible for updating the ships next torque and accelerate values
*******************************************************************************************************************/
    pub fn update_engine_vectors(&mut self) {        
        if self.target_heading.is_some() && self.target_lead_position.is_some() { // Calculate the fastest rotation curve current heading, target_heading
            let current_diff = angle_diff(heading(), (self.target_lead_position.unwrap() - position()).angle());
            if current_diff.abs() > 0.25 {
                self.next_torque = calculate_angular_velocity(50.0, current_diff);                
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
            accelerate(self.target_position.unwrap());            
        }
    }

pub fn turn_unit_test(&mut self) {
    if self.counter < 120.0 {
        self.counter += 1.0;
    } else {
        self.target_lead_position = Some(vec2(rand(-world_size(), world_size()),rand(-world_size(),world_size())));
        self.counter = 0.0;
    } 
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
        if self.objective == SEEK_AND_DESTROY {
            self.target_lead_position = Some(self.track(target(), target_velocity(), velocity()));  // update aim and tracking position
            self.target_position = Some(self.approach_and_orbit(250.0, 400.0, position(), self.target_lead_position.unwrap(), target_velocity())); // 300 meter orbit
        } else {
            self.turn_unit_test();
        }
        // calculate heading from lead position
        if self.target_lead_position.is_some() {
            self.target_heading = Some ((self.target_lead_position.unwrap() - position()).angle());
        }
        self.update_guns();
        self.update_engine_vectors();
        self.draw_diagnostics();
    }  

/********************************************************************************************************
* ** Diagnostics **
* Functions used to update the systems diagnostics
*********************************************************************************************************/
    pub fn draw_diagnostics(&mut self) {
        
        //debug!("Current State: {}", self.state);
        debug!("Angular Velocity: {}", angular_velocity());
        debug!("Counter: {}", self.counter);
        debug!("Ships Heading {}", heading());
        if self.target_heading.is_some() {
            debug!("Target Heading {}", self.target_heading.unwrap());
        }
        draw_line(position(), target(), 0x00ff00);
        if self.target_lead_position.is_some() {
            let lead_position = self.target_lead_position.unwrap();
            let dp = (lead_position - position());
            debug!("distance to target: {} meters", dp.length());
            debug!("time to target: {} seconds", dp.length() / BULLET_SPEED);
            draw_line(position(), lead_position, 0xff0000);
        }
    }
/*******************************************************************************************************/
}
