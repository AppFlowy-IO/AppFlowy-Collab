#![allow(clippy::upper_case_acronyms)]

use crate::error::DatabaseError;
use crate::fields::number_type_option::number_currency::Currency;
use crate::fields::{
  TypeOptionCellReader, TypeOptionCellWriter, TypeOptionData, TypeOptionDataBuilder,
};
use std::fmt::Display;

use collab::preclude::Any;

use crate::entity::FieldType;
use crate::rows::{Cell, new_cell_builder};
use crate::template::number_parse::NumberCellData;

use fancy_regex::Regex;
use lazy_static::lazy_static;
use rust_decimal::Decimal;
use rusty_money::{Money, define_currency_set};
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;
use std::str::FromStr;
use strum::IntoEnumIterator;
use strum_macros::EnumIter;
use yrs::encoding::serde::from_any;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NumberTypeOption {
  #[serde(default, deserialize_with = "number_format_from_i64")]
  pub format: NumberFormat,
  #[serde(default)]
  pub scale: u32,
  #[serde(default)]
  pub symbol: String,
  #[serde(default)]
  pub name: String,
}

impl Default for NumberTypeOption {
  fn default() -> Self {
    let format = NumberFormat::default();
    let symbol = format.symbol();
    NumberTypeOption {
      format,
      scale: 0,
      symbol,
      name: "Number".to_string(),
    }
  }
}

impl From<TypeOptionData> for NumberTypeOption {
  fn from(data: TypeOptionData) -> Self {
    from_any(&Any::from(data)).unwrap()
  }
}

impl From<NumberTypeOption> for TypeOptionData {
  fn from(data: NumberTypeOption) -> Self {
    TypeOptionDataBuilder::from([
      ("format".into(), Any::BigInt(data.format.value())),
      ("scale".into(), Any::BigInt(data.scale as i64)),
      ("name".into(), data.name.into()),
      ("symbol".into(), data.symbol.into()),
    ])
  }
}

impl TypeOptionCellReader for NumberTypeOption {
  fn json_cell(&self, cell: &Cell) -> Value {
    // Returns the formated number string.
    self.stringify_cell(cell).into()
  }

  fn numeric_cell(&self, cell: &Cell) -> Option<f64> {
    let cell_data = NumberCellData::from(cell);
    cell_data.0.parse::<f64>().ok()
  }

  fn convert_raw_cell_data(&self, text: &str) -> String {
    match self.format_cell_data(text) {
      Ok(cell_data) => cell_data.to_string(),
      Err(_) => "".to_string(),
    }
  }
}

impl TypeOptionCellWriter for NumberTypeOption {
  fn convert_json_to_cell(&self, json_value: Value) -> Cell {
    if let Some(data) = match json_value {
      Value::String(s) => Some(s),
      Value::Number(n) => Some(n.to_string()),
      _ => None,
    } {
      NumberCellData(data).into()
    } else {
      new_cell_builder(FieldType::Number)
    }
  }
}

impl NumberTypeOption {
  pub fn new() -> Self {
    Self::default()
  }

  pub fn format_cell_data<T: AsRef<str>>(
    &self,
    num_cell_data: T,
  ) -> Result<NumberCellFormat, DatabaseError> {
    match self.format {
      NumberFormat::Num => {
        if SCIENTIFIC_NOTATION_REGEX
          .is_match(num_cell_data.as_ref())
          .unwrap()
        {
          match Decimal::from_scientific(&num_cell_data.as_ref().to_lowercase()) {
            Ok(value, ..) => Ok(NumberCellFormat::from_decimal(value)),
            Err(_) => Ok(NumberCellFormat::new()),
          }
        } else {
          // Test the input string is start with dot and only contains number.
          // If it is, add a 0 before the dot. For example, ".123" -> "0.123"
          let num_str = match START_WITH_DOT_NUM_REGEX.captures(num_cell_data.as_ref()) {
            Ok(Some(captures)) => match captures.get(0).map(|m| m.as_str().to_string()) {
              Some(s) => {
                format!("0{}", s)
              },
              None => "".to_string(),
            },
            // Extract the number from the string.
            // For example, "123abc" -> "123". check out the number_type_option_input_test test for
            // more examples.
            _ => match EXTRACT_NUM_REGEX.captures(num_cell_data.as_ref()) {
              Ok(Some(captures)) => captures
                .get(0)
                .map(|m| m.as_str().to_string())
                .unwrap_or_default(),
              _ => "".to_string(),
            },
          };

          match Decimal::from_str(&num_str) {
            Ok(decimal, ..) => Ok(NumberCellFormat::from_decimal(decimal)),
            Err(_) => Ok(NumberCellFormat::new()),
          }
        }
      },
      _ => {
        // If the format is not number, use the format string to format the number.
        NumberCellFormat::from_format_str(num_cell_data.as_ref(), &self.format)
      },
    }
  }

  pub fn set_format(&mut self, format: NumberFormat) {
    self.format = format;
    self.symbol = format.symbol();
  }
}

fn number_format_from_i64<'de, D>(deserializer: D) -> Result<NumberFormat, D::Error>
where
  D: Deserializer<'de>,
{
  let value = i64::deserialize(deserializer)?;
  Ok(NumberFormat::from(value))
}

#[derive(Debug, Default)]
pub struct NumberCellFormat {
  decimal: Option<Decimal>,
  money: Option<String>,
}

impl NumberCellFormat {
  pub fn new() -> Self {
    Self {
      decimal: Default::default(),
      money: None,
    }
  }

  /// The num_str might contain currency symbol, e.g. $1,000.00
  pub fn from_format_str(num_str: &str, format: &NumberFormat) -> Result<Self, DatabaseError> {
    if num_str.is_empty() {
      return Ok(Self::default());
    }
    // If the first char is not '-', then it is a sign.
    let sign_positive = match num_str.find('-') {
      None => true,
      Some(offset) => offset != 0,
    };

    let num_str = auto_fill_zero_at_start_if_need(num_str);
    let num_str = extract_number(&num_str);
    match Decimal::from_str(&num_str) {
      Ok(mut decimal) => {
        decimal.set_sign_positive(sign_positive);
        let money = Money::from_decimal(decimal, format.currency());
        Ok(Self::from_money(money))
      },
      Err(_) => match Money::from_str(&num_str, format.currency()) {
        Ok(money) => Ok(Self::from_money(money)),
        Err(_) => Ok(Self::default()),
      },
    }
  }

  pub fn from_decimal(decimal: Decimal) -> Self {
    Self {
      decimal: Some(decimal),
      money: None,
    }
  }

  pub fn from_money(money: Money<Currency>) -> Self {
    Self {
      decimal: Some(*money.amount()),
      money: Some(money.to_string()),
    }
  }

  pub fn decimal(&self) -> &Option<Decimal> {
    &self.decimal
  }

  pub fn is_empty(&self) -> bool {
    self.decimal.is_none()
  }

  pub fn to_unformatted_string(&self) -> String {
    match self.decimal {
      None => String::default(),
      Some(decimal) => decimal.to_string(),
    }
  }
}

fn auto_fill_zero_at_start_if_need(num_str: &str) -> String {
  match START_WITH_DOT_NUM_REGEX.captures(num_str) {
    Ok(Some(captures)) => match captures.get(0).map(|m| m.as_str().to_string()) {
      Some(s) => format!("0{}", s),
      None => num_str.to_string(),
    },
    _ => num_str.to_string(),
  }
}

fn extract_number(num_str: &str) -> String {
  let mut matches = EXTRACT_NUM_REGEX.find_iter(num_str);
  let mut values = vec![];
  while let Some(Ok(m)) = matches.next() {
    values.push(m.as_str().to_string());
  }
  values.join("")
}

impl Display for NumberCellFormat {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    let str = match &self.money {
      None => match self.decimal {
        None => String::default(),
        Some(decimal) => decimal.to_string(),
      },
      Some(money) => money.to_string(),
    };
    write!(f, "{}", str)
  }
}

lazy_static! {
  static ref SCIENTIFIC_NOTATION_REGEX: Regex = Regex::new(r"([+-]?\d*\.?\d+)e([+-]?\d+)").unwrap();
  pub(crate) static ref EXTRACT_NUM_REGEX: Regex = Regex::new(r"-?\d+(\.\d+)?").unwrap();
  pub(crate) static ref START_WITH_DOT_NUM_REGEX: Regex = Regex::new(r"^\.\d+").unwrap();
  pub static ref CURRENCY_SYMBOL: Vec<String> = NumberFormat::iter()
    .map(|format| format.symbol())
    .collect::<Vec<String>>();
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, EnumIter, Serialize, Deserialize, Default)]
pub enum NumberFormat {
  #[default]
  Num = 0,
  USD = 1,
  CanadianDollar = 2,
  EUR = 4,
  Pound = 5,
  Yen = 6,
  Ruble = 7,
  Rupee = 8,
  Won = 9,
  Yuan = 10,
  Real = 11,
  Lira = 12,
  Rupiah = 13,
  Franc = 14,
  HongKongDollar = 15,
  NewZealandDollar = 16,
  Krona = 17,
  NorwegianKrone = 18,
  MexicanPeso = 19,
  Rand = 20,
  NewTaiwanDollar = 21,
  DanishKrone = 22,
  Baht = 23,
  Forint = 24,
  Koruna = 25,
  Shekel = 26,
  ChileanPeso = 27,
  PhilippinePeso = 28,
  Dirham = 29,
  ColombianPeso = 30,
  Riyal = 31,
  Ringgit = 32,
  Leu = 33,
  ArgentinePeso = 34,
  UruguayanPeso = 35,
  Percent = 36,
}

impl NumberFormat {
  pub fn value(&self) -> i64 {
    *self as i64
  }
}

impl From<i64> for NumberFormat {
  fn from(value: i64) -> Self {
    match value {
      0 => NumberFormat::Num,
      1 => NumberFormat::USD,
      2 => NumberFormat::CanadianDollar,
      4 => NumberFormat::EUR,
      5 => NumberFormat::Pound,
      6 => NumberFormat::Yen,
      7 => NumberFormat::Ruble,
      8 => NumberFormat::Rupee,
      9 => NumberFormat::Won,
      10 => NumberFormat::Yuan,
      11 => NumberFormat::Real,
      12 => NumberFormat::Lira,
      13 => NumberFormat::Rupiah,
      14 => NumberFormat::Franc,
      15 => NumberFormat::HongKongDollar,
      16 => NumberFormat::NewZealandDollar,
      17 => NumberFormat::Krona,
      18 => NumberFormat::NorwegianKrone,
      19 => NumberFormat::MexicanPeso,
      20 => NumberFormat::Rand,
      21 => NumberFormat::NewTaiwanDollar,
      22 => NumberFormat::DanishKrone,
      23 => NumberFormat::Baht,
      24 => NumberFormat::Forint,
      25 => NumberFormat::Koruna,
      26 => NumberFormat::Shekel,
      27 => NumberFormat::ChileanPeso,
      28 => NumberFormat::PhilippinePeso,
      29 => NumberFormat::Dirham,
      30 => NumberFormat::ColombianPeso,
      31 => NumberFormat::Riyal,
      32 => NumberFormat::Ringgit,
      33 => NumberFormat::Leu,
      34 => NumberFormat::ArgentinePeso,
      35 => NumberFormat::UruguayanPeso,
      36 => NumberFormat::Percent,
      _ => NumberFormat::Num,
    }
  }
}

define_currency_set!(
    number_currency {
        NUMBER : {
            code: "",
            exponent: 2,
            locale: EnEu,
            minor_units: 1,
            name: "number",
            symbol: "RUB",
            symbol_first: false,
        },
        PERCENT : {
            code: "",
            exponent: 2,
            locale: EnIn,
            minor_units: 1,
            name: "percent",
            symbol: "%",
            symbol_first: false,
        },
        USD : {
            code: "USD",
            exponent: 2,
            locale: EnUs,
            minor_units: 1,
            name: "United States Dollar",
            symbol: "$",
            symbol_first: true,
        },
        CANADIAN_DOLLAR : {
            code: "USD",
            exponent: 2,
            locale: EnUs,
            minor_units: 1,
            name: "Canadian Dollar",
            symbol: "CA$",
            symbol_first: true,
        },
         NEW_TAIWAN_DOLLAR : {
            code: "USD",
            exponent: 2,
            locale: EnUs,
            minor_units: 1,
            name: "NewTaiwan Dollar",
            symbol: "NT$",
            symbol_first: true,
        },
        HONG_KONG_DOLLAR : {
            code: "USD",
            exponent: 2,
            locale: EnUs,
            minor_units: 1,
            name: "HongKong Dollar",
            symbol: "HZ$",
            symbol_first: true,
        },
        NEW_ZEALAND_DOLLAR : {
            code: "USD",
            exponent: 2,
            locale: EnUs,
            minor_units: 1,
            name: "NewZealand Dollar",
            symbol: "NZ$",
            symbol_first: true,
        },
        EUR : {
            code: "EUR",
            exponent: 2,
            locale: EnEu,
            minor_units: 1,
            name: "Euro",
            symbol: "€",
            symbol_first: true,
        },
        GIP : {
            code: "GIP",
            exponent: 2,
            locale: EnUs,
            minor_units: 1,
            name: "Gibraltar Pound",
            symbol: "£",
            symbol_first: true,
        },
        CNY : {
            code: "CNY",
            exponent: 2,
            locale: EnUs,
            minor_units: 1,
            name: "Chinese Renminbi Yuan",
            symbol: "¥",
            symbol_first: true,
        },
        YUAN : {
            code: "CNY",
            exponent: 2,
            locale: EnUs,
            minor_units: 1,
            name: "Chinese Renminbi Yuan",
            symbol: "CN¥",
            symbol_first: true,
        },
        RUB : {
            code: "RUB",
            exponent: 2,
            locale: EnEu,
            minor_units: 1,
            name: "Russian Ruble",
            symbol: "RUB",
            symbol_first: false,
        },
        INR : {
            code: "INR",
            exponent: 2,
            locale: EnIn,
            minor_units: 50,
            name: "Indian Rupee",
            symbol: "₹",
            symbol_first: true,
        },
        KRW : {
            code: "KRW",
            exponent: 0,
            locale: EnUs,
            minor_units: 1,
            name: "South Korean Won",
            symbol: "₩",
            symbol_first: true,
        },
        BRL : {
            code: "BRL",
            exponent: 2,
            locale: EnUs,
            minor_units: 5,
            name: "Brazilian real",
            symbol: "R$",
            symbol_first: true,
        },
        TRY : {
            code: "TRY",
            exponent: 2,
            locale: EnEu,
            minor_units: 1,
            name: "Turkish Lira",
            // symbol: "₺",
            symbol: "TRY",
            symbol_first: true,
        },
        IDR : {
            code: "IDR",
            exponent: 2,
            locale: EnUs,
            minor_units: 5000,
            name: "Indonesian Rupiah",
            // symbol: "Rp",
            symbol: "IDR",
            symbol_first: true,
        },
        CHF : {
            code: "CHF",
            exponent: 2,
            locale: EnUs,
            minor_units: 5,
            name: "Swiss Franc",
            // symbol: "Fr",
            symbol: "CHF",
            symbol_first: true,
        },
        SEK : {
            code: "SEK",
            exponent: 2,
            locale: EnBy,
            minor_units: 100,
            name: "Swedish Krona",
            // symbol: "kr",
            symbol: "SEK",
            symbol_first: false,
        },
        NOK : {
            code: "NOK",
            exponent: 2,
            locale: EnUs,
            minor_units: 100,
            name: "Norwegian Krone",
            // symbol: "kr",
            symbol: "NOK",
            symbol_first: false,
        },
        MEXICAN_PESO : {
            code: "USD",
            exponent: 2,
            locale: EnUs,
            minor_units: 1,
            name: "Mexican Peso",
            symbol: "MX$",
            symbol_first: true,
        },
        ZAR : {
            code: "ZAR",
            exponent: 2,
            locale: EnUs,
            minor_units: 10,
            name: "South African Rand",
            // symbol: "R",
            symbol: "ZAR",
            symbol_first: true,
        },
        DKK : {
            code: "DKK",
            exponent: 2,
            locale: EnEu,
            minor_units: 50,
            name: "Danish Krone",
            // symbol: "kr.",
            symbol: "DKK",
            symbol_first: false,
        },
        THB : {
            code: "THB",
            exponent: 2,
            locale: EnUs,
            minor_units: 1,
            name: "Thai Baht",
            // symbol: "฿",
            symbol: "THB",
            symbol_first: true,
        },
        HUF : {
            code: "HUF",
            exponent: 0,
            locale: EnBy,
            minor_units: 5,
            name: "Hungarian Forint",
            // symbol: "Ft",
            symbol: "HUF",
            symbol_first: false,
        },
        KORUNA : {
            code: "CZK",
            exponent: 2,
            locale: EnBy,
            minor_units: 100,
            name: "Czech Koruna",
            // symbol: "Kč",
            symbol: "CZK",
            symbol_first: false,
        },
        SHEKEL : {
            code: "CZK",
            exponent: 2,
            locale: EnBy,
            minor_units: 100,
            name: "Czech Koruna",
            symbol: "Kč",
            symbol_first: false,
        },
        CLP : {
            code: "CLP",
            exponent: 0,
            locale: EnEu,
            minor_units: 1,
            name: "Chilean Peso",
            // symbol: "$",
            symbol: "CLP",
            symbol_first: true,
        },
        PHP : {
            code: "PHP",
            exponent: 2,
            locale: EnUs,
            minor_units: 1,
            name: "Philippine Peso",
            symbol: "₱",
            symbol_first: true,
        },
        AED : {
            code: "AED",
            exponent: 2,
            locale: EnUs,
            minor_units: 25,
            name: "United Arab Emirates Dirham",
            // symbol: "د.إ",
            symbol: "AED",
            symbol_first: false,
        },
        COP : {
            code: "COP",
            exponent: 2,
            locale: EnEu,
            minor_units: 20,
            name: "Colombian Peso",
            // symbol: "$",
            symbol: "COP",
            symbol_first: true,
        },
        SAR : {
            code: "SAR",
            exponent: 2,
            locale: EnUs,
            minor_units: 5,
            name: "Saudi Riyal",
            // symbol: "ر.س",
            symbol: "SAR",
            symbol_first: true,
        },
        MYR : {
            code: "MYR",
            exponent: 2,
            locale: EnUs,
            minor_units: 5,
            name: "Malaysian Ringgit",
            // symbol: "RM",
            symbol: "MYR",
            symbol_first: true,
        },
        RON : {
            code: "RON",
            exponent: 2,
            locale: EnEu,
            minor_units: 1,
            name: "Romanian Leu",
            // symbol: "ر.ق",
            symbol: "RON",
            symbol_first: false,
        },
        ARS : {
            code: "ARS",
            exponent: 2,
            locale: EnEu,
            minor_units: 1,
            name: "Argentine Peso",
            // symbol: "$",
            symbol: "ARS",
            symbol_first: true,
        },
        UYU : {
            code: "UYU",
            exponent: 2,
            locale: EnEu,
            minor_units: 100,
            name: "Uruguayan Peso",
            // symbol: "$U",
            symbol: "UYU",
            symbol_first: true,
        }
    }
);

impl NumberFormat {
  pub fn currency(&self) -> &'static number_currency::Currency {
    match self {
      NumberFormat::Num => number_currency::NUMBER,
      NumberFormat::USD => number_currency::USD,
      NumberFormat::CanadianDollar => number_currency::CANADIAN_DOLLAR,
      NumberFormat::EUR => number_currency::EUR,
      NumberFormat::Pound => number_currency::GIP,
      NumberFormat::Yen => number_currency::CNY,
      NumberFormat::Ruble => number_currency::RUB,
      NumberFormat::Rupee => number_currency::INR,
      NumberFormat::Won => number_currency::KRW,
      NumberFormat::Yuan => number_currency::YUAN,
      NumberFormat::Real => number_currency::BRL,
      NumberFormat::Lira => number_currency::TRY,
      NumberFormat::Rupiah => number_currency::IDR,
      NumberFormat::Franc => number_currency::CHF,
      NumberFormat::HongKongDollar => number_currency::HONG_KONG_DOLLAR,
      NumberFormat::NewZealandDollar => number_currency::NEW_ZEALAND_DOLLAR,
      NumberFormat::Krona => number_currency::SEK,
      NumberFormat::NorwegianKrone => number_currency::NOK,
      NumberFormat::MexicanPeso => number_currency::MEXICAN_PESO,
      NumberFormat::Rand => number_currency::ZAR,
      NumberFormat::NewTaiwanDollar => number_currency::NEW_TAIWAN_DOLLAR,
      NumberFormat::DanishKrone => number_currency::DKK,
      NumberFormat::Baht => number_currency::THB,
      NumberFormat::Forint => number_currency::HUF,
      NumberFormat::Koruna => number_currency::KORUNA,
      NumberFormat::Shekel => number_currency::SHEKEL,
      NumberFormat::ChileanPeso => number_currency::CLP,
      NumberFormat::PhilippinePeso => number_currency::PHP,
      NumberFormat::Dirham => number_currency::AED,
      NumberFormat::ColombianPeso => number_currency::COP,
      NumberFormat::Riyal => number_currency::SAR,
      NumberFormat::Ringgit => number_currency::MYR,
      NumberFormat::Leu => number_currency::RON,
      NumberFormat::ArgentinePeso => number_currency::ARS,
      NumberFormat::UruguayanPeso => number_currency::UYU,
      NumberFormat::Percent => number_currency::PERCENT,
    }
  }

  pub fn symbol(&self) -> String {
    self.currency().symbol.to_string()
  }
}

#[cfg(test)]
mod tests {
  use collab::util::AnyMapExt;

  use crate::template::entity::CELL_DATA;

  use super::*;
  /// Testing when the input is not a number.
  #[test]
  fn number_type_option_input_test() {
    let type_option = NumberTypeOption::default();

    // Input is empty String
    assert_number(&type_option, "", "");
    assert_number(&type_option, "abc", "");
    assert_number(&type_option, "-123", "-123");
    assert_number(&type_option, "abc-123", "-123");
    assert_number(&type_option, "+123", "123");
    assert_number(&type_option, "0.2", "0.2");
    assert_number(&type_option, "-0.2", "-0.2");
    assert_number(&type_option, "-$0.2", "0.2");
    assert_number(&type_option, ".2", "0.2");
  }

  #[test]
  fn dollar_type_option_test() {
    let mut type_option = NumberTypeOption::new();
    type_option.format = NumberFormat::USD;

    assert_number(&type_option, "", "");
    assert_number(&type_option, "abc", "");
    assert_number(&type_option, "-123", "-$123");
    assert_number(&type_option, "+123", "$123");
    assert_number(&type_option, "0.2", "$0.2");
    assert_number(&type_option, "-0.2", "-$0.2");
    assert_number(&type_option, "-$0.2", "-$0.2");
    assert_number(&type_option, "-€0.2", "-$0.2");
    assert_number(&type_option, ".2", "$0.2");
  }

  #[test]
  fn dollar_type_option_test2() {
    let mut type_option = NumberTypeOption::new();
    type_option.format = NumberFormat::USD;

    assert_number(&type_option, "99999999999", "$99,999,999,999");
    assert_number(&type_option, "$99,999,999,999", "$99,999,999,999");
  }
  #[test]
  fn other_symbol_to_dollar_type_option_test() {
    let mut type_option = NumberTypeOption::new();
    type_option.format = NumberFormat::USD;

    assert_number(&type_option, "€0.2", "$0.2");
    assert_number(&type_option, "-€0.2", "-$0.2");
    assert_number(&type_option, "-CN¥0.2", "-$0.2");
    assert_number(&type_option, "CN¥0.2", "$0.2");
    assert_number(&type_option, "0.2", "$0.2");
  }

  #[test]
  fn euro_type_option_test() {
    let mut type_option = NumberTypeOption::new();
    type_option.format = NumberFormat::EUR;

    assert_number(&type_option, "0.2", "€0,2");
    assert_number(&type_option, "1000", "€1.000");
    assert_number(&type_option, "1234.56", "€1.234,56");
  }

  fn assert_number(type_option: &NumberTypeOption, input_str: &str, expected_str: &str) {
    let output = type_option.convert_raw_cell_data(input_str);
    assert_eq!(output, expected_str.to_owned());
  }

  #[test]
  fn number_cell_to_serde() {
    let number_type_option = NumberTypeOption::default();
    let cell_writer: Box<dyn TypeOptionCellReader> = Box::new(number_type_option);
    {
      let mut cell: Cell = new_cell_builder(FieldType::Number);
      cell.insert(CELL_DATA.into(), "42".into());
      let serde_val = cell_writer.json_cell(&cell);
      assert_eq!(serde_val, Value::String("42".into()));
    }
  }

  #[test]
  fn number_serde_to_cell() {
    let number_type_option = NumberTypeOption::default();
    let cell_writer: Box<dyn TypeOptionCellWriter> = Box::new(number_type_option);
    {
      // js string
      let cell: Cell = cell_writer.convert_json_to_cell(Value::String("42.195".to_string()));
      let data = cell.get_as::<String>(CELL_DATA).unwrap();
      assert_eq!(data, "42.195");
    }
    {
      // js number
      let cell: Cell = cell_writer.convert_json_to_cell(Value::Number(10.into()));
      let data = cell.get_as::<String>(CELL_DATA).unwrap();
      assert_eq!(data, "10");
    }
  }
}
