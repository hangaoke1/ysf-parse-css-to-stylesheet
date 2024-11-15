

use std::{cell::RefCell, collections::HashMap, rc::Rc};

use lightningcss::{printer::PrinterOptions, properties::{animation, Property}, traits::ToCss, values::{easing::EasingFunction, time}};

use crate::{generate_expr_lit_num, generate_expr_lit_str, generate_invalid_expr, style_parser::KeyFrameItem, visitor::parse_style_values};
use swc_core::{common::DUMMY_SP, ecma::ast::*};
use super::{traits::ToExpr, unit::{Platform, PropertyTuple}};

#[derive(Debug, Clone)]
pub struct Animation {
  pub id: String,
  pub keyframes: Option<Rc<RefCell<HashMap<String, Vec<KeyFrameItem>>>>>,
  pub animation_name: Option<String>,
  pub animation_duration: Option<f32>,
  pub animation_delay: Option<f32>,
  pub animation_iteration: Option<f32>,
  pub animation_timeing_function: Option<EasingFunction>
}

impl From<(String, &Property<'_>, Option<Rc<RefCell<HashMap<String, Vec<KeyFrameItem>>>>>)> for Animation {
  fn from(value: (String, &Property<'_>, Option<Rc<RefCell<HashMap<String, Vec<KeyFrameItem>>>>>)) -> Self {

    let mut animation_name = None;
    let mut animation_duration =  None; // 0.0
    let mut animation_delay =  None; // 0.0
    let mut animation_iteration =  None; // 1.0
    let mut animation_timeing_function = None; // EasingFunction::Ease
    
    match value.1 {
      // Property::AnimationName(_, _) => todo!(),
      // Property::AnimationDuration(_, _) => todo!(),
      // Property::AnimationTimingFunction(_, _) => todo!(),
      // Property::AnimationIterationCount(_, _) => todo!(),
      // Property::AnimationDirection(_, _) => todo!(),
      // Property::AnimationPlayState(_, _) => todo!(),
      // Property::AnimationDelay(_, _) => todo!(),
      // Property::AnimationFillMode(_, _) => todo!(),
      Property::Animation(animation_list, _) => {
        animation_list.into_iter().for_each(|animation| {
          animation_name = Some(animation.name.to_css_string(PrinterOptions::default()).unwrap());
          animation_duration = Some(match animation.duration {
            time::Time::Seconds(s) => s,
            time::Time::Milliseconds(m) => m * 60.0,
          });
          animation_delay = Some(match animation.delay {
            time::Time::Seconds(s) => s,
            time::Time::Milliseconds(m) => m * 60.0,
          });
          animation_iteration = Some(match animation.iteration_count {
            animation::AnimationIterationCount::Number(num) => num,
            animation::AnimationIterationCount::Infinite => -1.0,
          });

          animation_timeing_function = Some(animation.timing_function.clone());
        });
      },
      Property::AnimationDelay(delay, _) => {
        animation_delay = Some(match delay.get(0).unwrap() {
          time::Time::Seconds(s) => *s,
          time::Time::Milliseconds(m) => m * 60.0,
        });
      },
      Property::AnimationDuration(duration, _) => {
        animation_duration = Some(match duration.get(0).unwrap() {
          time::Time::Seconds(s) => *s,
          time::Time::Milliseconds(m) => m * 60.0,
        })
      },
      Property::AnimationIterationCount(iteration, _) => {
        animation_iteration = Some(match iteration.get(0).unwrap() {
          animation::AnimationIterationCount::Number(num) => *num,
          animation::AnimationIterationCount::Infinite => -1.0,
        })
      },
      Property::AnimationName(name, _) => {
        animation_name = Some(name.to_css_string(PrinterOptions::default()).unwrap())
      },
      Property::AnimationTimingFunction(timing_function, _) => {
        animation_timeing_function = Some(timing_function.get(0).unwrap().clone());
      },
      _ => {}
    }
    
    Animation {
      id: value.0,
      keyframes: value.2.clone(),
      animation_name,
      animation_duration,
      animation_delay,
      animation_iteration,
      animation_timeing_function
    }

  }
}


impl ToExpr for Animation {
  fn to_expr(&self) -> PropertyTuple {

    let mut exprs = vec![];
    if let Some(delay) = self.animation_delay {
      exprs.push(("animationDelay".to_string(), generate_expr_lit_num!((delay * 1000.0) as f64)))
    }
    if let Some(iteration) = self.animation_iteration {
      exprs.push(("animationIterationCount".to_string(), generate_expr_lit_num!(iteration as f64)))
    }
    if let Some(duration) = self.animation_duration {
      exprs.push(("animationDuration".to_string(), generate_expr_lit_num!((duration * 1000.0) as f64)))
    }
    if let Some(timeing_function) = &self.animation_timeing_function {
      exprs.push(("animationTimeingFunction".to_string(), generate_expr_lit_str!(timeing_function.to_css_string(PrinterOptions::default()).unwrap())))
    }
    if let Some(name) = &self.animation_name {
      if let Some(keframes) = &self.keyframes {

      let keyframe_map = keframes.borrow();
      if let Some(keyframe_items) = keyframe_map.get(name) {
        // animation-name: keyframes
        exprs.push(("animationName".to_string(), Expr::Array(ArrayLit {
          span: DUMMY_SP,
          elems: keyframe_items.into_iter().map(|item| {
            return Some(ExprOrSpread {
              spread: None,
              expr: Box::new(Expr::Object(ObjectLit {
                span: DUMMY_SP,
                props: vec![
                  PropOrSpread::Prop(Box::new(Prop::KeyValue(KeyValueProp {
                    key: PropName::Str("percentage".into()),
                    value: Box::new(generate_expr_lit_num!(item.percentage as f64))
                  }))),                   
                  PropOrSpread::Prop(Box::new(Prop::KeyValue(KeyValueProp {
                    key: PropName::Str("event".into()),
                    value: Box::new(Expr::Object(ObjectLit {
                      span: DUMMY_SP,
                      props: parse_style_values(item.declarations.clone(), Platform::Harmony)
                    }))
                  })))
                ]
              }))
            })
          }).collect::<Vec<Option<ExprOrSpread>>>()
        })))
        
        // let mut mut_percentage = 0.0;
        // return PropertyTuple::One(
        //   "animation".to_string(),
        //   Expr::Object(ObjectLit {
        //     span: DUMMY_SP,
        //     props: vec![
        //       PropOrSpread::Prop(Box::new(Prop::KeyValue(KeyValueProp {
        //         key: PropName::Str("params".into()),
        //         value: Box::new(Expr::Object(ObjectLit {
        //           span: DUMMY_SP,
        //           props: vec![
        //             PropOrSpread::Prop(Box::new(Prop::KeyValue(KeyValueProp {
        //               key: PropName::Str("delay".into()),
        //               value: Box::new(generate_expr_lit_num!((self.animation_delay * 1000.0) as f64))
        //             }))),
        //             PropOrSpread::Prop(Box::new(Prop::KeyValue(KeyValueProp {
        //               key: PropName::Str("iterations".into()),
        //               value: Box::new(generate_expr_lit_num!(self.animation_iteration as f64))
        //             }))),
        //             PropOrSpread::Prop(Box::new(Prop::KeyValue(KeyValueProp {
        //               key: PropName::Str("duration".into()),
        //               value: Box::new(generate_expr_lit_num!((self.animation_duration * 1000.0) as f64))
        //             }))),
        //           ]
        //         }))
        //       }))),
        //       PropOrSpread::Prop(Box::new(Prop::KeyValue(KeyValueProp {
        //         key: PropName::Str("keyframes".into()),
        //         value: Box::new(Expr::Array(ArrayLit {
        //           span: DUMMY_SP,
        //           elems: keyframe_items.into_iter().map(|item| {
        //             let item_duration = (item.percentage - mut_percentage) * self.animation_duration * 1000.0;
        //             mut_percentage = item.percentage;
        //             return Some(ExprOrSpread {
        //               spread: None,
        //               expr: Box::new(Expr::Object(ObjectLit {
        //                 span: DUMMY_SP,
        //                 props: vec![
        //                   PropOrSpread::Prop(Box::new(Prop::KeyValue(KeyValueProp {
        //                     key: PropName::Str("percentage".into()),
        //                     value: Box::new(generate_expr_lit_num!(item.percentage as f64))
        //                   }))),
        //                   PropOrSpread::Prop(Box::new(Prop::KeyValue(KeyValueProp {
        //                     key: PropName::Str("duration".into()),
        //                     value: Box::new(generate_expr_lit_num!(item_duration as f64))
        //                   }))),
        //                   PropOrSpread::Prop(Box::new(Prop::KeyValue(KeyValueProp {
        //                     key: PropName::Str("curve".into()),
        //                     value: Box::new(generate_expr_lit_str!(self.animation_timeing_function.to_css_string(PrinterOptions::default()).unwrap()))
        //                   }))),                        
        //                   PropOrSpread::Prop(Box::new(Prop::KeyValue(KeyValueProp {
        //                     key: PropName::Str("event".into()),
        //                     value: Box::new(Expr::Object(ObjectLit {
        //                       span: DUMMY_SP,
        //                       props: parse_style_values(item.declarations.clone(), Platform::Harmony)
        //                     }))
        //                   })))
        //                 ]
        //               }))
        //             })
        //           }).collect::<Vec<Option<ExprOrSpread>>>()
        //         }))
        //       })))
        //     ]
        //   })
        // )
      
        }
      }
    }

    PropertyTuple::Array(exprs)
  }

  fn to_rn_expr(&self) -> PropertyTuple {
    PropertyTuple::One(
      self.id.to_string(),
      generate_invalid_expr!()
    )
  }
}

