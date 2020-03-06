use crate::ff::PrimeField;

use crate::plonk::domains::Domain;
use crate::multicore::Worker;
use crate::plonk::fft::distribute_powers;
use super::*;
use crate::plonk::fft::cooley_tukey_ntt::{bitreverse, log2_floor};

pub struct PrecomputedOmegas<F: PrimeField> {
    pub omegas: Vec<F>,
    pub coset: Vec<F>,
    pub omegas_inv: Vec<F>,
    domain_size: usize
}

impl<F: PrimeField> PrecomputedOmegas<F> {
    pub fn new_for_domain(domain: &Domain<F>, worker: &Worker) -> Self {
        let domain_size = domain.size as usize;
        let precomputation_size = domain_size/2;

        let omega = domain.generator;
        let omega_inv = domain.generator.inverse().expect("must exist");

        let mut omegas = vec![F::zero(); domain_size];
        let mut omegas_inv = vec![F::zero(); precomputation_size];

        worker.scope(omegas.len(), |scope, chunk| {
            for (i, v) in omegas.chunks_mut(chunk).enumerate() {
                scope.spawn(move |_| {
                    let mut u = omega.pow(&[(i * chunk) as u64]);
                    for v in v.iter_mut() {
                        *v = u;
                        u.mul_assign(&omega);
                    }
                });
            }
        });

        worker.scope(omegas_inv.len(), |scope, chunk| {
            for (i, v) in omegas_inv.chunks_mut(chunk).enumerate() {
                scope.spawn(move |_| {
                    let mut u = omega_inv.pow(&[(i * chunk) as u64]);
                    for v in v.iter_mut() {
                        *v = u;
                        u.mul_assign(&omega_inv);
                    }
                });
            }
        });

        let mut coset = omegas.clone();
        let mult_generator = F::multiplicative_generator();

        worker.scope(coset.len(), |scope, chunk| {
            for v in coset.chunks_mut(chunk) {
                scope.spawn(move |_| {
                    for v in v.iter_mut() {
                        v.mul_assign(&mult_generator);
                    }
                });
            }
        });

        PrecomputedOmegas{
            omegas,
            coset,
            omegas_inv,
            domain_size
        }
    }
}

impl<F: PrimeField> FriPrecomputations<F> for PrecomputedOmegas<F>{
    fn new_for_domain_size(size: usize) -> Self {
        let domain = Domain::<F>::new_for_size(size as u64).expect("domain must exist");
        let worker = Worker::new();
        Self::new_for_domain(&domain, &worker)
    }

    fn omegas_inv_ref(&self) -> &[F] {
        &self.omegas_inv[..]
    }

    fn domain_size(&self) -> usize {
        self.domain_size
    }
}

impl<F: PrimeField> FftPrecomputations<F> for PrecomputedOmegas<F>{
    fn new_for_domain_size(size: usize) -> Self {
        let domain = Domain::<F>::new_for_size(size as u64).expect("domain must exist");
        let worker = Worker::new();
        Self::new_for_domain(&domain, &worker)
    }

    fn element_for_index(&self, index: usize) -> &F {
        &self.omegas_inv[index]
    }

    fn domain_size(&self) -> usize {
        self.domain_size
    }
}

pub struct PrecomputedInvOmegas<F: PrimeField> {
    pub omegas_inv: Vec<F>,
    domain_size: usize
}

impl<F: PrimeField> PrecomputedInvOmegas<F> {
    pub fn new_for_domain(domain: &Domain<F>, worker: &Worker) -> Self {
        let domain_size = domain.size as usize;
        let precomputation_size = domain_size/2;

        let omega = domain.generator;
        let omega_inv = omega.inverse().expect("must exist");

        let mut omegas_inv = vec![F::zero(); precomputation_size];

        worker.scope(omegas_inv.len(), |scope, chunk| {
            for (i, v) in omegas_inv.chunks_mut(chunk).enumerate() {
                scope.spawn(move |_| {
                    let mut u = omega_inv.pow(&[(i * chunk) as u64]);
                    for v in v.iter_mut() {
                        *v = u;
                        u.mul_assign(&omega_inv);
                    }
                });
            }
        });

        PrecomputedInvOmegas{
            omegas_inv,
            domain_size
        }
    }
}

impl<F: PrimeField> FriPrecomputations<F> for PrecomputedInvOmegas<F>{
    fn new_for_domain_size(size: usize) -> Self {
        let domain = Domain::<F>::new_for_size(size as u64).expect("domain must exist");
        let worker = Worker::new();
        Self::new_for_domain(&domain, &worker)
    }

    fn omegas_inv_ref(&self) -> &[F] {
        &self.omegas_inv[..]
    }

    fn domain_size(&self) -> usize {
        self.domain_size
    }
}

pub struct OmegasInvBitreversed<F: PrimeField> {
    pub omegas: Vec<F>,
    domain_size: usize
}

impl<F: PrimeField> OmegasInvBitreversed<F> {
    pub fn new_for_domain(domain: &Domain<F>, worker: &Worker) -> Self {
        let domain_size = domain.size as usize;
        
        let omega = domain.generator.inverse().expect("must exist");
        let precomputation_size = domain_size / 2;

        let log_n = log2_floor(precomputation_size);

        let mut omegas = vec![F::zero(); precomputation_size];

        worker.scope(omegas.len(), |scope, chunk| {
            for (i, v) in omegas.chunks_mut(chunk).enumerate() {
                scope.spawn(move |_| {
                    let mut u = omega.pow(&[(i * chunk) as u64]);
                    for v in v.iter_mut() {
                        *v = u;
                        u.mul_assign(&omega);
                    }
                });
            }
        });

        if omegas.len() > 2 {
            for k in 0..omegas.len() {
                let rk = bitreverse(k, log_n as usize);
                if k < rk {
                    omegas.swap(rk, k);
                }
            }
        }

        OmegasInvBitreversed{
            omegas,
            domain_size
        }
    }
}

impl<F: PrimeField> FriPrecomputations<F> for OmegasInvBitreversed<F> {
    fn new_for_domain_size(size: usize) -> Self {
        let domain = Domain::<F>::new_for_size(size as u64).expect("domain must exist");
        let worker = Worker::new();
        Self::new_for_domain(&domain, &worker)
    }

    fn omegas_inv_bitreversed(&self) -> &[F] {
        &self.omegas[..]
    }

    fn domain_size(&self) -> usize {
        self.domain_size
    }
}


mod ct {
    use super::OmegasInvBitreversed;
    use crate::ff::PrimeField;

    use crate::plonk::domains::Domain;
    use crate::multicore::Worker;
    use crate::plonk::fft::cooley_tukey_ntt::*;

    impl<F: PrimeField> CTPrecomputations<F> for OmegasInvBitreversed<F> {
        fn new_for_domain_size(size: usize) -> Self {
            let domain = Domain::<F>::new_for_size(size as u64).expect("domain must exist");
            let worker = Worker::new();
            Self::new_for_domain(&domain, &worker)
        }

        fn bit_reversed_omegas(&self) -> &[F] {
            &self.omegas[..]
        }

        fn domain_size(&self) -> usize {
            self.domain_size
        }
    }
}


pub struct CosetOmegasInvBitreversed<F: PrimeField> {
    pub omegas: Vec<F>,
    domain_size: usize
}

impl<F: PrimeField> CosetOmegasInvBitreversed<F> {
    pub fn new_for_domain(domain: &Domain<F>, worker: &Worker) -> Self {
        let domain_size = domain.size as usize;
        
        let omega = domain.generator.inverse().expect("must exist");
        let precomputation_size = domain_size / 2;

        let log_n = log2_floor(precomputation_size);

        let mut omegas = vec![F::zero(); precomputation_size];
        let geninv = F::multiplicative_generator().inverse().expect("must exist");

        worker.scope(omegas.len(), |scope, chunk| {
            for (i, v) in omegas.chunks_mut(chunk).enumerate() {
                scope.spawn(move |_| {
                    let mut u = omega.pow(&[(i * chunk) as u64]);
                    u.mul_assign(&geninv);
                    for v in v.iter_mut() {
                        *v = u;
                        u.mul_assign(&omega);
                    }
                });
            }
        });

        if omegas.len() > 2 {
            for k in 0..omegas.len() {
                let rk = bitreverse(k, log_n as usize);
                if k < rk {
                    omegas.swap(rk, k);
                }
            }
        }

        CosetOmegasInvBitreversed{
            omegas,
            domain_size
        }
    }
}

impl<F: PrimeField> FriPrecomputations<F> for CosetOmegasInvBitreversed<F> {
    fn new_for_domain_size(size: usize) -> Self {
        let domain = Domain::<F>::new_for_size(size as u64).expect("domain must exist");
        let worker = Worker::new();
        Self::new_for_domain(&domain, &worker)
    }

    fn omegas_inv_bitreversed(&self) -> &[F] {
        &self.omegas[..]
    }

    fn domain_size(&self) -> usize {
        self.domain_size
    }
}