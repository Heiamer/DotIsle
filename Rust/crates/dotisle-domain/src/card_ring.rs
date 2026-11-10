//! Periodic coordinates for the overview deck. Motion never encounters an endpoint.
pub const COUNT:usize=5;
pub const STEP:f64=360.;
pub const CYCLE:f64=STEP*COUNT as f64;
pub fn phase(position:f64)->f64{position.rem_euclid(CYCLE)/STEP}
pub fn index(position:f64)->usize{(position/STEP).round().rem_euclid(COUNT as f64)as usize}
pub fn slot_index(slot:i64)->usize{slot.rem_euclid(COUNT as i64)as usize}
pub fn snap(position:f64)->f64{(position/STEP).round()*STEP}
/// Choose the closest occurrence of a card, including across the last/first seam.
pub fn nearest(position:f64,card:usize)->f64{
 let base=(card%COUNT)as f64*STEP;base+((position-base)/CYCLE).round()*CYCLE
}
/// Rebase all motion coordinates together, preserving displacement and velocity.
pub fn rebase_offset(position:f64)->f64{
 if position.abs()>CYCLE*32.{(position/CYCLE).floor()*CYCLE}else{0.}
}
#[cfg(test)]mod tests{
 use super::*;
 #[test]fn both_directions_keep_the_order_across_many_laps(){
  for step in -1000i32..1000{let p=step as f64*STEP;assert_eq!(index(p),step.rem_euclid(COUNT as i32)as usize);assert_eq!(index(p+STEP),(index(p)+1)%COUNT);assert_eq!(index(p-STEP),(index(p)+COUNT-1)%COUNT);}
 }
 #[test]fn selecting_a_neighbour_crosses_the_seam_in_one_step(){
  assert_eq!(nearest(4.*STEP,0),CYCLE);assert_eq!(nearest(0.,4),-STEP);
  for lap in -20..20{let origin=lap as f64*CYCLE;for card in 0..COUNT{let p=origin+card as f64*STEP;assert!((nearest(p,(card+1)%COUNT)-p-STEP).abs()<1e-9);}}
 }
 #[test]fn phase_is_continuous_at_the_loop_and_rebase_preserves_motion(){
  for p in [-100.*CYCLE-0.25,100.*CYCLE+0.25]{let shift=rebase_offset(p);assert!(shift.abs()>0.);assert!((phase(p)-phase(p-shift)).abs()<1e-9);let target=p+STEP*1.4;assert!(((target-shift)-(p-shift)-(target-p)).abs()<1e-9);assert_eq!(index(p),index(p-shift));}
  assert!(phase(-0.01)>4.99);assert!(phase(CYCLE+0.01)<0.01);
 }
}
