

use lightningcss::properties::{display::{Display::{Keyword, Pair}, DisplayInside, DisplayKeyword, DisplayOutside}, Property};

use crate::{generate_expr_lit_str, generate_invalid_expr};

use super::{traits::ToExpr, unit::PropertyTuple};

#[derive(Debug, Clone)]
pub struct Display {
  pub id: String,
  pub value: EnumValue
}

#[derive(Debug, Clone)]
pub enum EnumValue {
  None,
  Flex,
  Block,
  Invalid,
}

impl From<(String, &Property<'_>)> for Display {
  fn from(value: (String, &Property<'_>)) -> Self {
    Display {
      id: value.0,
      value: {
        if let Property::Display(value) = &value.1 {
          match &value {
            Keyword(value) => match &value {
              DisplayKeyword::None => EnumValue::None,
              _ => EnumValue::Invalid,
            },
            Pair(value) => {
              if let DisplayInside::Flex(_) = value.inside {
                EnumValue::Flex
              } else {
                if let DisplayOutside::Block = value.outside {
                  EnumValue::Block
                } else {
                  EnumValue::Invalid
                }
              }
            }
          }
        } else {
          EnumValue::Invalid
        }
      }
    }
  }
}


impl ToExpr for Display {
  fn to_expr(&self) -> PropertyTuple {
    PropertyTuple::One(
      self.id.to_string(),
      match &self.value {
        EnumValue::None => generate_expr_lit_str!("none"),
        EnumValue::Flex => generate_expr_lit_str!("flex"),
        EnumValue::Block => generate_expr_lit_str!("block"),
        EnumValue::Invalid => generate_invalid_expr!(),
      }
    )
  }

  fn to_rn_expr(&self) -> PropertyTuple {
    PropertyTuple::One(
      self.id.to_string(),
      match &self.value {
        EnumValue::None => generate_expr_lit_str!("none"),
        EnumValue::Flex => generate_expr_lit_str!("flex"),
        EnumValue::Block => generate_expr_lit_str!("block"),
        EnumValue::Invalid => generate_invalid_expr!(),
      }
    )
  }
}

