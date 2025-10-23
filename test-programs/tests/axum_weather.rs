use anyhow::Result;
use serde_json::Value;

const CITY: &str = "portland";
const COUNT: usize = 2;

#[test_log::test]
fn weather() -> Result<()> {
    // Run wasmtime serve.
    let _serve = test_programs::WasmtimeServe::new(test_programs::axum::WEATHER)?;

    // TEST /weather weather handler
    let body = ureq::get(format!(
        "http://127.0.0.1:8081/weather?city={CITY}&count={COUNT}"
    ))
    .call()?
    .body_mut()
    .read_json::<Value>()?;
    let array = body.as_array().expect("json body is an array");
    assert_eq!(array.len(), COUNT);
    let item_0 = &array[0];
    let loc_0 = item_0
        .get("location")
        .expect("item 0 has `location`")
        .as_object()
        .expect("location 0 is object");
    let qn_0 = loc_0
        .get("qualified_name")
        .expect("location has qualified name")
        .as_str()
        .expect("name is string");
    assert!(
        qn_0.contains("Multnomah"),
        "{qn_0:?} should contain substring 'Multnomah'"
    );

    let item_1 = &array[1];
    let loc_1 = item_1
        .get("location")
        .expect("item 1 has `location`")
        .as_object()
        .expect("location 1 is object");
    let qn_1 = loc_1
        .get("qualified_name")
        .expect("location has qualified name")
        .as_str()
        .expect("name is string");
    assert!(
        qn_1.contains("Cumberland"),
        "{qn_1:?} should contain substring 'Cumberland'"
    );

    Ok(())
}
