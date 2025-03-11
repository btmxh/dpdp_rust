use chrono::{Duration, Utc};
use dpdp_rust::simulation::simulator::Simulator;
use rand::{rngs::SmallRng, SeedableRng};

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    let mut rng = SmallRng::seed_from_u64(727);
    let mut sim = Simulator::new(&mut rng, 1)?;
    // sim.simulate_until(Utc::now().naive_utc() + Duration::hours(2));
    sim.simulate_until(Utc::now().naive_utc() + Duration::hours(2));
    Ok(())
}
