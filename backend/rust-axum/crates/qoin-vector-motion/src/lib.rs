use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

const MAX_PARTICLES: usize = 2_000;
const GRID_SIZE: f32 = 0.16;

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
pub struct ParticleInput {
    pub id: u32,
    pub x: f32,
    pub y: f32,
    #[serde(default)]
    pub vx: f32,
    #[serde(default)]
    pub vy: f32,
    #[serde(default = "default_weight")]
    pub weight: f32,
}

fn default_weight() -> f32 { 1.0 }

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq)]
pub struct ParticleOutput {
    pub id: u32,
    pub x: f32,
    pub y: f32,
    pub vx: f32,
    pub vy: f32,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
pub struct MotionConfig {
    pub bullish: f32,
    pub bearish: f32,
    pub iterations: u16,
    pub repulsion: f32,
    pub damping: f32,
}

impl Default for MotionConfig {
    fn default() -> Self {
        Self { bullish: 0.0, bearish: 0.0, iterations: 48, repulsion: 0.012, damping: 0.88 }
    }
}

#[derive(Debug, PartialEq)]
pub enum MotionError {
    TooManyParticles,
    NonFiniteInput,
    InvalidConfig,
    DuplicateParticleId,
}

fn cell(value: f32) -> i32 { (value / GRID_SIZE).floor() as i32 }

/// Deterministic 2D force integration for the desktop vector-space renderer.
///
/// A uniform spatial hash limits repulsion checks to nearby cells instead of
/// comparing every particle with every other particle. Complexity is O(n*k)
/// for locally bounded density rather than an unconditional O(n²) pass.
pub fn simulate(
    particles: &[ParticleInput],
    config: MotionConfig,
) -> Result<Vec<ParticleOutput>, MotionError> {
    if particles.len() > MAX_PARTICLES { return Err(MotionError::TooManyParticles); }
    let mut ids = HashSet::with_capacity(particles.len());
    if particles.iter().any(|particle| !ids.insert(particle.id)) {
        return Err(MotionError::DuplicateParticleId);
    }
    if particles.iter().any(|p| ![p.x,p.y,p.vx,p.vy,p.weight].iter().all(|v| v.is_finite())) {
        return Err(MotionError::NonFiniteInput);
    }
    if ![config.bullish,config.bearish,config.repulsion,config.damping].iter().all(|v| v.is_finite())
        || config.repulsion < 0.0 || !(0.0..=1.0).contains(&config.damping) {
        return Err(MotionError::InvalidConfig);
    }

    let mut positions: Vec<[f32; 2]> = particles.iter().map(|p| [p.x.clamp(-2.0,2.0), p.y.clamp(-2.0,2.0)]).collect();
    let mut velocities: Vec<[f32; 2]> = particles.iter().map(|p| [p.vx,p.vy]).collect();
    let iterations = config.iterations.clamp(1, 240);
    let bullish = config.bullish.clamp(-1.0, 1.0);
    let bearish = config.bearish.clamp(-1.0, 1.0);
    // Bound force magnitude even when a caller supplies an extreme finite value.
    let repulsion = config.repulsion.clamp(0.0, 0.25);

    for _ in 0..iterations {
        let mut grid: HashMap<(i32,i32), Vec<usize>> = HashMap::with_capacity(particles.len());
        for (index, p) in positions.iter().enumerate() {
            grid.entry((cell(p[0]),cell(p[1]))).or_default().push(index);
        }
        let mut next = positions.clone();
        for i in 0..positions.len() {
            let p = positions[i];
            let gx = cell(p[0]); let gy = cell(p[1]);
            let mut fx = bullish * 0.0015 - p[0] * 0.0008;
            let mut fy = bearish * 0.0015 - p[1] * 0.0008;
            for dx in -1..=1 {
                for dy in -1..=1 {
                    if let Some(neighbors) = grid.get(&(gx+dx,gy+dy)) {
                        for &j in neighbors {
                            if i == j { continue; }
                            let ox = p[0] - positions[j][0];
                            let oy = p[1] - positions[j][1];
                            let distance_sq = ox*ox + oy*oy;
                            if distance_sq < GRID_SIZE*GRID_SIZE {
                                if distance_sq <= 1.0e-7 {
                                    // Coincident particles need a stable separating direction.
                                    // Derive it from the unordered ID pair so the force is
                                    // deterministic and equal/opposite for the two particles.
                                    let low = particles[i].id.min(particles[j].id);
                                    let high = particles[i].id.max(particles[j].id);
                                    let hash = low.wrapping_mul(0x9E37_79B9)
                                        ^ high.wrapping_mul(0x85EB_CA6B);
                                    let directions = [
                                        [1.0_f32, 0.0_f32],
                                        [0.70710677, 0.70710677],
                                        [0.0, 1.0],
                                        [-0.70710677, 0.70710677],
                                        [-1.0, 0.0],
                                        [-0.70710677, -0.70710677],
                                        [0.0, -1.0],
                                        [0.70710677, -0.70710677],
                                    ];
                                    let direction = directions[(hash as usize) & 7];
                                    let sign = if particles[i].id < particles[j].id { 1.0 } else { -1.0 };
                                    let strength = repulsion * 18.0;
                                    fx += direction[0] * sign * strength;
                                    fy += direction[1] * sign * strength;
                                } else {
                                    let inverse = distance_sq.sqrt().recip();
                                    let strength = repulsion * inverse.min(18.0);
                                    fx += ox * inverse * strength;
                                    fy += oy * inverse * strength;
                                }
                            }
                        }
                    }
                }
            }
            let weight = particles[i].weight.clamp(0.1, 4.0);
            velocities[i][0] = (velocities[i][0] + fx * weight).clamp(-0.035,0.035) * config.damping;
            velocities[i][1] = (velocities[i][1] + fy * weight).clamp(-0.035,0.035) * config.damping;
            next[i][0] = (p[0] + velocities[i][0]).clamp(-1.5,1.5);
            next[i][1] = (p[1] + velocities[i][1]).clamp(-1.5,1.5);
        }
        positions = next;
    }

    Ok(particles.iter().enumerate().map(|(i,p)| ParticleOutput {
        id: p.id, x: positions[i][0], y: positions[i][1], vx: velocities[i][0], vy: velocities[i][1]
    }).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministic_and_bounded() {
        let particles = vec![
            ParticleInput { id: 1, x: 0.0, y: 0.0, vx: 0.0, vy: 0.0, weight: 1.0 },
            ParticleInput { id: 2, x: 0.01, y: 0.01, vx: 0.0, vy: 0.0, weight: 1.0 },
        ];
        let config = MotionConfig { bullish: 0.7, bearish: 0.2, ..Default::default() };
        let first = simulate(&particles, config).unwrap();
        let second = simulate(&particles, config).unwrap();
        assert_eq!(first.len(), 2);
        assert_eq!(first[0].x, second[0].x);
        assert!(first.iter().all(|p| p.x.abs() <= 1.5 && p.y.abs() <= 1.5));
        assert_ne!(first[0].x, first[1].x);
    }

    #[test]
    fn rejects_non_finite_inputs() {
        let particles = [ParticleInput { id: 1, x: f32::NAN, y: 0.0, vx: 0.0, vy: 0.0, weight: 1.0 }];
        assert_eq!(simulate(&particles, MotionConfig::default()), Err(MotionError::NonFiniteInput));
    }

    #[test]
    fn separates_coincident_particles_deterministically() {
        let particles = [
            ParticleInput { id: 11, x: 0.0, y: 0.0, vx: 0.0, vy: 0.0, weight: 1.0 },
            ParticleInput { id: 22, x: 0.0, y: 0.0, vx: 0.0, vy: 0.0, weight: 1.0 },
        ];
        let config = MotionConfig { bullish: 0.0, bearish: 0.0, iterations: 8, ..Default::default() };
        let first = simulate(&particles, config).unwrap();
        let second = simulate(&particles, config).unwrap();

        assert_eq!(first, second);
        assert_ne!((first[0].x, first[0].y), (first[1].x, first[1].y));
        let dx = first[0].x - particles[0].x;
        let dy = first[0].y - particles[0].y;
        let other_dx = first[1].x - particles[1].x;
        let other_dy = first[1].y - particles[1].y;
        assert!((dx + other_dx).abs() < 1.0e-6);
        assert!((dy + other_dy).abs() < 1.0e-6);
    }

    #[test]
    fn rejects_duplicate_particle_ids() {
        let particles = [
            ParticleInput { id: 7, x: 0.0, y: 0.0, vx: 0.0, vy: 0.0, weight: 1.0 },
            ParticleInput { id: 7, x: 0.2, y: 0.2, vx: 0.0, vy: 0.0, weight: 1.0 },
        ];
        assert_eq!(
            simulate(&particles, MotionConfig::default()),
            Err(MotionError::DuplicateParticleId)
        );
    }

    #[test]
    fn extreme_finite_repulsion_keeps_output_finite_and_bounded() {
        let particles = [
            ParticleInput { id: 1, x: 0.0, y: 0.0, vx: 0.0, vy: 0.0, weight: 1.0 },
            ParticleInput { id: 2, x: 0.001, y: 0.001, vx: 0.0, vy: 0.0, weight: 1.0 },
        ];
        let config = MotionConfig { repulsion: f32::MAX, ..Default::default() };
        let output = simulate(&particles, config).unwrap();
        assert!(output.iter().all(|p| {
            p.x.is_finite() && p.y.is_finite() && p.vx.is_finite() && p.vy.is_finite()
                && p.x.abs() <= 1.5 && p.y.abs() <= 1.5
        }));
    }

    #[test]
    fn enforces_particle_limit() {
        let particles = vec![ParticleInput { id: 0, x: 0.0, y: 0.0, vx: 0.0, vy: 0.0, weight: 1.0 }; MAX_PARTICLES+1];
        assert_eq!(simulate(&particles, MotionConfig::default()), Err(MotionError::TooManyParticles));
    }
}
