//! ECDSA Circuit Implementation using the Pallas Curve
//! 
//! The Pallas curve is a prime-order curve designed for zero-knowledge proof systems.
//! It is part of the Pasta curves family and has the following parameters:
//! 
//! - Base field characteristic (p): 
//!   0x40000000000000000000000000000000224698fc094cf91b992d30ed00000001
//! 
//! - Scalar field characteristic (r):
//!   0x40000000000000000000000000000000224698fc0994a8dd8c46eb2100000001
//! 
//! - Curve equation: y² = x³ + 5 (where a = 0, b = 5)
//! 
//! The curve is designed for efficient implementation and high security,
//! providing approximately 128 bits of security.

use halo2_proofs::{
    arithmetic::{Field, CurveAffine},
    circuit::{Layouter, SimpleFloorPlanner, Value, AssignedCell, Region},
    plonk::{Circuit, ConstraintSystem, Column, Advice, 
        Instance, Selector, Expression, ErrorFront},
    poly::Rotation,
};

use ff::PrimeField;
use group::Curve;
use rand::rngs::OsRng;
use pasta_curves::{
    pallas::{Base, Point, Scalar},
};


fn curve_scalar<C: CurveAffine>(value: u64) -> C::Scalar {
    C::Scalar::from_u128(value as u128)
}

fn base_to_scalar<C: CurveAffine>(value: &C::Base) -> C::Scalar {
    let value_bits: Vec<bool> = value.to_repr()
        .as_ref()
        .iter()
        .flat_map(|byte| (0..8).map(move |i| (byte >> i) & 1 == 1))
        .collect();

    let mut acc = C::Scalar::ZERO;
    for bit in value_bits.iter() {
        acc = acc + acc;
        if *bit {
            acc = acc + C::Scalar::ONE;
        }
    }
    acc
}

// Circuit configuration
#[derive(Debug, Clone)]
struct EcdsaConfig {
    q_enable: Selector,
    x: Column<Advice>,     // point x coordinate
    y: Column<Advice>,     // point y coordinate
    r: Column<Advice>,     // signature r
    s: Column<Advice>,     // signature s
    w: Column<Advice>,     // witness for s inverse
    hash: Column<Instance>, // message hash
}

// Chip to handle curve operations
struct EcdsaChip<C: CurveAffine> {
    config: EcdsaConfig,
    _marker: std::marker::PhantomData<C>,
}

impl<C: CurveAffine> EcdsaChip<C> {
    fn construct(config: EcdsaConfig) -> Self {
        Self {
            config,
            _marker: std::marker::PhantomData,
        }
    }

    fn assign_point(
        &self,
        region: &mut Region<'_, C::Scalar>,
        column: Column<Advice>,
        offset: usize,
        value: Value<C::Scalar>,
    ) -> Result<AssignedCell<C::Scalar, C::Scalar>, ErrorFront> {
        region.assign_advice(
            || "point coordinate",
            column,
            offset,
            || value,
        )
    }

    fn check_s_nonzero(
        &self,
        s: Value<C::Scalar>,
    ) -> Value<C::Scalar> {
        s.map(|s| s.invert().unwrap())
    }

    fn point_double(
        &self,
        region: &mut Region<'_, C::Scalar>,
        point: (AssignedCell<C::Scalar, C::Scalar>, AssignedCell<C::Scalar, C::Scalar>),
    ) -> Result<(AssignedCell<C::Scalar, C::Scalar>, AssignedCell<C::Scalar, C::Scalar>), ErrorFront> {
        let (x, y) = point;
        
        let x_value = x.value().copied();
        let y_value = y.value().copied();
        
        let lambda = x_value.zip(y_value).map(|(x, y)| {
            let xx = x * x;
            let two_y = y + y;
            let three_xx = xx + xx + xx;
            three_xx * two_y.invert().unwrap()
        });
    
        let x_r = lambda.zip(x_value).map(|(l, x)| {
            l * l - x - x
        });
    
        let y_r = lambda.zip(x_value).zip(x_r).zip(y_value)
            .map(|(((l, x), xr), y)| {
                l * (x - xr) - y
            });

        let x_r_cell = region.assign_advice(
            || "x_double",
            self.config.x,
            1,
            || x_r,
        )?;

        let y_r_cell = region.assign_advice(
            || "y_double",
            self.config.y,
            1,
            || y_r,
        )?;

        Ok((x_r_cell, y_r_cell))
    }

    fn point_add(
        &self,
        region: &mut Region<'_, C::Scalar>,
        p1: (AssignedCell<C::Scalar, C::Scalar>, AssignedCell<C::Scalar, C::Scalar>),
        p2: (AssignedCell<C::Scalar, C::Scalar>, AssignedCell<C::Scalar, C::Scalar>),
    ) -> Result<(AssignedCell<C::Scalar, C::Scalar>, AssignedCell<C::Scalar, C::Scalar>), ErrorFront> {
        let (x1, y1) = p1;
        let (x2, y2) = p2;

        let x1_value = x1.value().copied();
        let y1_value = y1.value().copied();
        let x2_value = x2.value().copied();
        let y2_value = y2.value().copied();

        let lambda = x2_value.zip(x1_value).zip(y2_value).zip(y1_value).map(
            |(((x2, x1), y2), y1)| {
                let dy = y2 - y1;
                let dx = x2 - x1;
                dy * dx.invert().unwrap()
            }
        );

        let x_r = lambda.zip(x1_value).zip(x2_value)
            .map(|((l, x1), x2)| l * l - x1 - x2);
            
        let y_r = lambda.zip(x1_value).zip(x_r).zip(y1_value)
            .map(|(((l, x1), xr), y1)| l * (x1 - xr) - y1);

        let x_r_cell = region.assign_advice(
            || "x_add",
            self.config.x,
            2,
            || x_r,
        )?;

        let y_r_cell = region.assign_advice(
            || "y_add",
            self.config.y,
            2,
            || y_r,
        )?;

        Ok((x_r_cell, y_r_cell))
    }

    fn scalar_mult(
        &self,
        region: &mut Region<'_, C::Scalar>,
        scalar: AssignedCell<C::Scalar, C::Scalar>,
        point: (AssignedCell<C::Scalar, C::Scalar>, AssignedCell<C::Scalar, C::Scalar>),
    ) -> Result<(AssignedCell<C::Scalar, C::Scalar>, AssignedCell<C::Scalar, C::Scalar>), ErrorFront> {
        let current = point.clone();
        let scalar_value = scalar.value().copied();

        // Convert scalar to binary
        let mut acc = current.clone();
        scalar_value.map(|s| {
            let mut bits: Vec<bool> = Vec::new();
            let mut s = s;
            for _ in 0..C::Scalar::NUM_BITS {
                bits.push(s.is_odd().into());
                let two = C::Scalar::from(2u64);
                s = s * two.invert().unwrap();
            }
            bits.reverse();

            // Process each bit
            for &bit in bits.iter().skip(1) {
                // Double
                acc = self.point_double(region, acc.clone())?;

                if bit {
                    // Add if current bit is 1
                    acc = self.point_add(region, acc.clone(), current.clone())?;
                }
            }
            Ok::<_, ErrorFront>(())
        });

        Ok(acc)
    }
}

// ECDSA circuit structure
#[derive(Default)]
struct EcdsaCircuit<C: CurveAffine> {
    // Public inputs
    public_key: Option<C>,
    message_hash: Option<C::Scalar>,
    
    // Private inputs (witness)
    signature: Option<(C::Scalar, C::Scalar)>, // (r, s)
}

// Circuit implementation
impl<C: CurveAffine> Circuit<C::Scalar> for EcdsaCircuit<C> {
    type Config = EcdsaConfig;
    type FloorPlanner = SimpleFloorPlanner;

    fn without_witnesses(&self) -> Self {
        Self::default()
    }

    fn configure(meta: &mut ConstraintSystem<C::Scalar>) -> Self::Config {
        let x = meta.advice_column();
        let y = meta.advice_column();
        let r = meta.advice_column();
        let s = meta.advice_column();
        let w = meta.advice_column();
        let hash = meta.instance_column();
        let q_enable = meta.selector();

        meta.enable_equality(x);
        meta.enable_equality(y);
        meta.enable_equality(r);
        meta.enable_equality(s);
        meta.enable_equality(hash);

        meta.create_gate("ecdsa_verify", |meta| {
            let q_enable = meta.query_selector(q_enable);
            let x = meta.query_advice(x, Rotation::cur());
            let y = meta.query_advice(y, Rotation::cur());
            let r = meta.query_advice(r, Rotation::cur());
            let s = meta.query_advice(s, Rotation::cur());
            let w = meta.query_advice(w, Rotation::cur());
            let hash = meta.query_instance(hash, Rotation::cur());

            // For Pallas curve, a = 0, b = 5
            let b = curve_scalar::<C>(5u64); 

            vec![
                // s ≠ 0: Check s * w = 1
                q_enable.clone() * (s.clone() * w.clone() - Expression::Constant(C::Scalar::ONE)), 
                
                // Point on curve: y² = x³ + ax + b
                q_enable.clone() * (
                    y.clone() * y.clone() - 
                    (x.clone() * x.clone() * x.clone() + 
                    Expression::Constant(b)) 
                ),
                
                // Verify r = R.x mod n
                q_enable.clone() * (r.clone() - x.clone()),
            ]
        });

        EcdsaConfig { q_enable, x, y, r, s, w, hash }
    }

    fn synthesize(
        &self,
        config: Self::Config,
        mut layouter: impl Layouter<C::Scalar>,
    ) -> Result<(), ErrorFront> {
        let chip: EcdsaChip<C> = EcdsaChip::construct(config);
    
        // Create separate namespace for main assignments
        let (u1, u2, g_coords, pk_coords) = layouter.namespace(|| "main assignments")
            .assign_region(
                || "ecdsa verify",
                |mut region| {
                    chip.config.q_enable.enable(&mut region, 0)?;
    
                    // 1. Assign public key
                    let pk_x = Value::known(self.public_key
                        .map(|pk| base_to_scalar::<C>(pk.coordinates().unwrap().x()))
                        .ok_or(ErrorFront::Synthesis)?);
        
                    let pk_y = Value::known(self.public_key
                        .map(|pk| base_to_scalar::<C>(pk.coordinates().unwrap().y()))
                        .ok_or(ErrorFront::Synthesis)?);
        
                    let pk_x = region.assign_advice(
                        || "pk_x",
                        chip.config.x,
                        0,
                        || pk_x,
                    )?;
            
                    let pk_y = region.assign_advice(
                        || "pk_y",
                        chip.config.y,
                        0,
                        || pk_y,
                    )?;
    
                    // 2. Assign signature
                    let (r, s) = self.signature.ok_or(ErrorFront::Synthesis)?;
                    let r_cell = region.assign_advice(
                        || "r",
                        chip.config.r,
                        0,
                        || Value::known(r),
                    )?;
    
                    let s_cell = region.assign_advice(
                        || "s",
                        chip.config.s,
                        0,
                        || Value::known(s),
                    )?;
    
                    // 3. ECDSA verification
                    let s_inv = s_cell.value().map(|s| s.invert().unwrap());
    
                    // u1 = hash * s⁻¹
                    let message_hash = self.message_hash.ok_or(ErrorFront::Synthesis)?;
                    let u1 = region.assign_advice(
                        || "u1",
                        chip.config.x,
                        1,
                        || Value::known(message_hash)
                            .zip(s_inv)
                            .map(|(h, s_inv)| h * s_inv),
                    )?;
    
                    // u2 = r * s⁻¹
                    let u2 = region.assign_advice(
                        || "u2",
                        chip.config.y,
                        1,
                        || Value::known(r).zip(s_inv).map(|(r, s_inv)| r * s_inv),
                    )?;
    
                    // R = u1*G + u2*PK
                    let generator = C::generator();
                    let g_x = Value::known(base_to_scalar::<C>(generator.coordinates().unwrap().x()));
                    let g_y = Value::known(base_to_scalar::<C>(generator.coordinates().unwrap().y()));
        
                    // Assign generator coordinates first
                    let g_x_cell = region.assign_advice(
                        || "g_x",
                        chip.config.x,
                        2,
                        || g_x,
                    )?;
        
                    let g_y_cell = region.assign_advice(
                        || "g_y",
                        chip.config.y,
                        2,
                        || g_y,
                    )?;
    
                    Ok((u1, u2, (g_x_cell, g_y_cell), (pk_x, pk_y)))
                },
            )?;
    
        // Scalar multiplications in separate namespaces
        let g_mult = layouter.namespace(|| "g_mult")
            .assign_region(
                || "scalar mult g",
                |mut region| chip.scalar_mult(&mut region, &u1, &g_coords),
            )?;
    
        let pk_mult = layouter.namespace(|| "pk_mult")
            .assign_region(
                || "scalar mult pk",
                |mut region| chip.scalar_mult(&mut region, &u2, &pk_coords),
            )?;
    
        // Final point addition in separate namespace
        layouter.namespace(|| "final addition")
            .assign_region(
                || "point addition",
                |mut region| {
                    let r_point = chip.point_add(&mut region, &g_mult, &pk_mult)?;
                    Ok(())
                },
            )?;
    
        Ok(())
    }
}

fn main() {
    println!("ECDSA Circuit implementation");
}

#[cfg(test)]
mod tests {
    use super::*;
    use halo2_proofs::dev::MockProver;
    use pasta_curves::pallas;

    #[test]
    fn test_ecdsa_verify() {
        let mut rng = OsRng;
        
        // Generate key pair
        let private_key = pallas::Scalar::random(&mut rng);
        let public_key = (pallas::Point::generator() * private_key).to_affine();
        
        // Generate message hash
        let msg_hash = pallas::Scalar::random(&mut rng);
        
        // Generate signature
        let k = pallas::Scalar::random(&mut rng);
        let r = (pallas::Point::generator() * k).to_affine().coordinates().unwrap().x();
        let s = k.invert().unwrap() * (msg_hash + (r * private_key));

        // Create circuit with real values
        let circuit = EcdsaCircuit::<pallas::Point> {
            public_key: Some(public_key),
            message_hash: Some(msg_hash),
            signature: Some((r, s)),
        };

        let prover = MockProver::run(
            8,  // k (circuit size parameter)
            &circuit,
            vec![vec![msg_hash.to_repr()]],
        ).unwrap();

        assert_eq!(prover.verify(), Ok(()));
    }

    #[test]
    fn test_invalid_signature() {
        let mut rng = OsRng;
        
        // Generate key pair
        let private_key = pallas::Scalar::random(&mut rng);
        let public_key = (pallas::Point::generator() * private_key).to_affine();
        
        // Generate message hash
        let msg_hash = pallas::Scalar::random(&mut rng);
        
        // Generate invalid signature
        let k = pallas::Scalar::random(&mut rng);
        let r = (pallas::Point::generator() * k).to_affine().coordinates().unwrap().x();
        let s = pallas::Scalar::random(&mut rng); // Random s instead of valid one

        // Create circuit with invalid signature
        let circuit = EcdsaCircuit::<pallas::Point> {
            public_key: Some(public_key),
            message_hash: Some(msg_hash),
            signature: Some((r, s)),
        };

        let prover = MockProver::run(
            8,
            &circuit,
            vec![vec![msg_hash.to_repr()]],
        ).unwrap();

        assert!(prover.verify().is_err());
    }
}