use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
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
                            if distance_sq > 1.0e-7 && distance_sq < GRID_SIZE*GRID_SIZE {
                                let inverse = distance_sq.sqrt().recip();
                                let strength = config.repulsion * inverse.min(18.0);
                                fx += ox * inverse * strength;
                                fy += oy * inverse * strength;
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
    fn enforces_particle_limit() {
        let particles = vec![ParticleInput { id: 0, x: 0.0, y: 0.0, vx: 0.0, vy: 0.0, weight: 1.0 }; MAX_PARTICLES+1];
        assert_eq!(simulate(&particles, MotionConfig::default()), Err(MotionError::TooManyParticles));
    }
}
