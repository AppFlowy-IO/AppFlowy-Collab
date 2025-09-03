use collab_entity::uuid_validation::*;
use uuid::Uuid;

#[test]
fn test_uuid_conversions() {
    // Test 1: Converting valid UUID strings
    let uuid_str = "550e8400-e29b-41d4-a716-446655440000";
    let db_id = try_parse_database_id(uuid_str).expect("Should parse valid UUID");
    assert_eq!(db_id.to_string(), uuid_str);
    
    // Test 2: Test serialization/deserialization
    let json = serde_json::to_string(&db_id).unwrap();
    assert_eq!(json, format!("\"{}\"", uuid_str), "Should serialize as string");
    
    
    // Test 4: All ID type conversions
    let view_id = view_id_from_any_string("view_1");
    let workspace_id = Uuid::new_v5(&Uuid::NAMESPACE_OID, "workspace_1".as_bytes());
    let db_view_id = Uuid::new_v5(&Uuid::NAMESPACE_OID, "db_view_1".as_bytes());
    let doc_id = document_id_from_any_string("doc_1");
    let block_id = block_id_from_any_string("block_1");
    
    // Verify they produce consistent UUIDs
    assert_eq!(view_id_from_any_string("view_1"), view_id);
    assert_eq!(Uuid::new_v5(&Uuid::NAMESPACE_OID, "workspace_1".as_bytes()), workspace_id);
    assert_eq!(Uuid::new_v5(&Uuid::NAMESPACE_OID, "db_view_1".as_bytes()), db_view_id);
    assert_eq!(document_id_from_any_string("doc_1"), doc_id);
    assert_eq!(block_id_from_any_string("block_1"), block_id);
    
    println!("✅ All UUID conversion tests passed!");
}
