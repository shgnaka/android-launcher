use quiet_launcher::weather::Location;
#[test]
fn geographic_boundaries_are_accepted() {
    for (input, lat, lon) in [
        ("-90,-180", -90., -180.),
        ("90,180", 90., 180.),
        (" 35.0 , 135.0 ", 35., 135.),
        ("0,0", 0., 0.),
    ] {
        let p = Location::parse(input).unwrap();
        assert_eq!(p.latitude, lat);
        assert_eq!(p.longitude, lon);
    }
}
#[test]
fn invalid_and_nonfinite_geography_is_rejected_before_request() {
    for input in [
        "90.001,0",
        "-90.001,0",
        "0,180.001",
        "0,-180.001",
        "NaN,0",
        "0,NaN",
        "inf,0",
        "0,-inf",
        "35",
        "35,135,0",
        ",135",
    ] {
        assert!(Location::parse(input).is_err(), "{input}");
    }
}
