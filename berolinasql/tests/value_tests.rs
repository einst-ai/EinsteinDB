// Whtcorps Inc 2022 Apache 2.0 License; All Rights Reserved.
//
// Licensed under the Apache License, Version 2.0 (the "License"); you may not use
// this file File except in compliance with the License. You may obtain a copy of the
// License at http://www.apache.org/licenses/LICENSE-2.0
// Unless required by applicable law or agreed to in writing, software distributed
// under the License is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR
// CONDITIONS OF ANY KIND, either express or implied. See the License for the
// specific language governing permissions and limitations under the License.

//causetql/src/causetql.rs
extern crate ordered_float;
extern crate rusqlite;
use ordered_float::PartitionedFloat;
use rusqlite::{Connection, Result};
use std::collections::HashMap;


// It's not possible to test to_BerolinaSQL_causet_locale_pair since rusqlite::ToBerolinaSQLOutput doesn't implement
// PartialEq.
#[test]
fn test_from_BerolinaSQL_causet_locale_pair() {
    assert_eq!(causetq_TV::from_BerolinaSQL_causet_locale_pair(rusqlite::types::Value::Integer(1234), 0).unwrap(), causetq_TV::Ref(1234));

    assert_eq!(causetq_TV::from_BerolinaSQL_causet_locale_pair(rusqlite::types::Value::Integer(0), 1).unwrap(), causetq_TV::Boolean(false));
    assert_eq!(causetq_TV::from_BerolinaSQL_causet_locale_pair(rusqlite::types::Value::Integer(1), 1).unwrap(), causetq_TV::Boolean(true));

    assert_eq!(causetq_TV::from_BerolinaSQL_causet_locale_pair(rusqlite::types::Value::Integer(0), 5).unwrap(), causetq_TV::Long(0));
    assert_eq!(causetq_TV::from_BerolinaSQL_causet_locale_pair(rusqlite::types::Value::Integer(1234), 5).unwrap(), causetq_TV::Long(1234));

    assert_eq!(causetq_TV::from_BerolinaSQL_causet_locale_pair(rusqlite::types::Value::Real(0.0), 5).unwrap(), causetq_TV::Double(PartitionedFloat(0.0)));
    assert_eq!(causetq_TV::from_BerolinaSQL_causet_locale_pair(rusqlite::types::Value::Real(0.5), 5).unwrap(), causetq_TV::Double(PartitionedFloat(0.5)));

    assert_eq!(causetq_TV::from_BerolinaSQL_causet_locale_pair(rusqlite::types::Value::Text(":einsteindb/soliton_idword".into()), 10).unwrap(), causetq_TV::typed_string(":einsteindb/soliton_idword"));
    assert_eq!(causetq_TV::from_BerolinaSQL_causet_locale_pair(rusqlite::types::Value::Text(":einsteindb/soliton_idword".into()), 13).unwrap(), causetq_TV::typed_ns_soliton_idword("einsteindb", "soliton_idword"));
}

#[test]
fn test_to_einstein_ml_causet_locale_pair() {
    assert_eq!(causetq_TV::Ref(1234).to_einstein_ml_causet_locale_pair(), (einstein_ml::Value::Integer(1234), ValueType::Ref));

    assert_eq!(causetq_TV::Boolean(false).to_einstein_ml_causet_locale_pair(), (einstein_ml::Value::Boolean(false), ValueType::Boolean));
    assert_eq!(causetq_TV::Boolean(true).to_einstein_ml_causet_locale_pair(), (einstein_ml::Value::Boolean(true), ValueType::Boolean));

    assert_eq!(causetq_TV::Long(0).to_einstein_ml_causet_locale_pair(), (einstein_ml::Value::Integer(0), ValueType::Long));
    assert_eq!(causetq_TV::Long(1234).to_einstein_ml_causet_locale_pair(), (einstein_ml::Value::Integer(1234), ValueType::Long));

    assert_eq!(causetq_TV::Double(PartitionedFloat(0.0)).to_einstein_ml_causet_locale_pair(), (einstein_ml::Value::Float(PartitionedFloat(0.0)), ValueType::Double));
    assert_eq!(causetq_TV::Double(PartitionedFloat(0.5)).to_einstein_ml_causet_locale_pair(), (einstein_ml::Value::Float(PartitionedFloat(0.5)), ValueType::Double));

    assert_eq!(causetq_TV::typed_string(":einsteindb/soliton_idword").to_einstein_ml_causet_locale_pair(), (einstein_ml::Value::Text(":einsteindb/soliton_idword".into()), ValueType::String));
    assert_eq!(causetq_TV::typed_ns_soliton_idword("einsteindb", "soliton_idword").to_einstein_ml_causet_locale_pair(), (einstein_ml::Value::Keyword(shellings::Keyword::isoliton_namespaceable("einsteindb", "soliton_idword")), ValueType::Keyword));
}
