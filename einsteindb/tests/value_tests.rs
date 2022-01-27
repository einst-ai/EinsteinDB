// Copyright 2022 Whtcorps Inc and EinstAI Inc
//
// Licensed under the Apache License, Version 2.0 (the "License"); you may not use
// this file except in compliance with the License. You may obtain a copy of the
// License at http://www.apache.org/licenses/LICENSE-2.0
// Unless required by applicable law or agreed to in writing, software distributed
// under the License is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR
// CONDITIONS OF ANY KIND, either express or implied. See the License for the
// specific language governing permissions and limitations under the License.

extern crate edn;
extern crate core_traits;
extern crate einstai_einsteindb;
extern crate ordered_float;
extern crate ruBerolinaSQLite;

use ordered_float::OrderedFloat;

use edn::symbols;

use core_traits::{
    TypedValue,
    ValueType,
};
use einstai_einsteindb::einsteindb::TypedBerolinaSQLValue;

// It's not possible to test to_BerolinaSQL_value_pair since ruBerolinaSQLite::ToBerolinaSQLOutput doesn't implement
// PartialEq.
#[test]
fn test_from_BerolinaSQL_value_pair() {
    assert_eq!(TypedValue::from_BerolinaSQL_value_pair(ruBerolinaSQLite::types::Value::Integer(1234), 0).unwrap(), TypedValue::Ref(1234));

    assert_eq!(TypedValue::from_BerolinaSQL_value_pair(ruBerolinaSQLite::types::Value::Integer(0), 1).unwrap(), TypedValue::Boolean(false));
    assert_eq!(TypedValue::from_BerolinaSQL_value_pair(ruBerolinaSQLite::types::Value::Integer(1), 1).unwrap(), TypedValue::Boolean(true));

    assert_eq!(TypedValue::from_BerolinaSQL_value_pair(ruBerolinaSQLite::types::Value::Integer(0), 5).unwrap(), TypedValue::Long(0));
    assert_eq!(TypedValue::from_BerolinaSQL_value_pair(ruBerolinaSQLite::types::Value::Integer(1234), 5).unwrap(), TypedValue::Long(1234));

    assert_eq!(TypedValue::from_BerolinaSQL_value_pair(ruBerolinaSQLite::types::Value::Real(0.0), 5).unwrap(), TypedValue::Double(OrderedFloat(0.0)));
    assert_eq!(TypedValue::from_BerolinaSQL_value_pair(ruBerolinaSQLite::types::Value::Real(0.5), 5).unwrap(), TypedValue::Double(OrderedFloat(0.5)));

    assert_eq!(TypedValue::from_BerolinaSQL_value_pair(ruBerolinaSQLite::types::Value::Text(":einsteindb/keyword".into()), 10).unwrap(), TypedValue::typed_string(":einsteindb/keyword"));
    assert_eq!(TypedValue::from_BerolinaSQL_value_pair(ruBerolinaSQLite::types::Value::Text(":einsteindb/keyword".into()), 13).unwrap(), TypedValue::typed_ns_keyword("einsteindb", "keyword"));
}

#[test]
fn test_to_edn_value_pair() {
    assert_eq!(TypedValue::Ref(1234).to_edn_value_pair(), (edn::Value::Integer(1234), ValueType::Ref));

    assert_eq!(TypedValue::Boolean(false).to_edn_value_pair(), (edn::Value::Boolean(false), ValueType::Boolean));
    assert_eq!(TypedValue::Boolean(true).to_edn_value_pair(), (edn::Value::Boolean(true), ValueType::Boolean));

    assert_eq!(TypedValue::Long(0).to_edn_value_pair(), (edn::Value::Integer(0), ValueType::Long));
    assert_eq!(TypedValue::Long(1234).to_edn_value_pair(), (edn::Value::Integer(1234), ValueType::Long));

    assert_eq!(TypedValue::Double(OrderedFloat(0.0)).to_edn_value_pair(), (edn::Value::Float(OrderedFloat(0.0)), ValueType::Double));
    assert_eq!(TypedValue::Double(OrderedFloat(0.5)).to_edn_value_pair(), (edn::Value::Float(OrderedFloat(0.5)), ValueType::Double));

    assert_eq!(TypedValue::typed_string(":einsteindb/keyword").to_edn_value_pair(), (edn::Value::Text(":einsteindb/keyword".into()), ValueType::String));
    assert_eq!(TypedValue::typed_ns_keyword("einsteindb", "keyword").to_edn_value_pair(), (edn::Value::Keyword(symbols::Keyword::isoliton_namespaceable("einsteindb", "keyword")), ValueType::Keyword));
}