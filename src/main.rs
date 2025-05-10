use chrono::{Duration, Local, NaiveTime};
use dpdp_rust::{
    callbacks::log_dispatch::LogDispatchCallback,
    model::{
        factory_info::FactoryId,
        route_info::{RouteInfo, RouteMap},
        vehicle_info::VehicleId,
    },
    simulation::simulator::{Simulator, VehicleInitialPosition},
};
use rand::rngs::SmallRng;

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    // let mut rng = SmallRng::seed_from_u64(727);
    // let mut sim = Simulator::new(VehicleInitialPosition::Random(&mut rng), 2)?;
    let mut sim = Simulator::new(
        VehicleInitialPosition::<SmallRng>::Deterministic(
            [
                ("V_1", "e2d5093fbe36431f8986ddb0e1c586be"),
                ("V_2", "7fe14b93f0f04ee7a994ef5b2c1fdb72"),
                ("V_3", "fa366fc87a124d32926daa5bb093129f"),
                ("V_4", "e47399648fa842b2b8f80094343d8091"),
                ("V_5", "becb4f85393540b287e7329758b8d832"),
            ]
            .map(|(vid, fid)| (VehicleId(vid.to_string()), FactoryId(fid.to_string())))
            .into(),
        ),
        1,
        vec![Box::new(LogDispatchCallback::new("test".into()))],
    )?;
    sim.simulate_until(
        Local::now().date_naive().and_time(NaiveTime::MIN) + Duration::minutes(200000),
    );
    // sim.simulate_until(Utc::now().naive_utc() + Duration::hours(2));
    Ok(())
}
