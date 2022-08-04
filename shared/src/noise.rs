// All credit to FastNoise2 creator Auburn (Jordan Peck) for the awesome work!
// This is merely an unofficial Rust port of some of the functions.
// https://github.com/Auburn/FastNoise2

use simdeez::{Simd, avx2::Avx2, scalar::Scalar, sse41::{Sse2, Sse41}};

pub struct Simplex;

impl Simplex {
    // Generates N*N*N values
    // Output strictly in [-1, 1]
    pub fn gen_3d<const N: u32>(start_x: i32, start_y: i32, start_z: i32, freq: f32, seed: i32, out: &mut [f32]) {
        debug_assert_eq!(out.len(), (N*N*N) as usize);

        if is_x86_feature_detected!("avx2") {
            unsafe { Simplex::gen_3d_impl::<Avx2, N>(start_x, start_y, start_z, freq, seed, out); }
        } else if is_x86_feature_detected!("sse4.1") {
            unsafe { Simplex::gen_3d_impl::<Sse41, N>(start_x, start_y, start_z, freq, seed, out); }
        } else if is_x86_feature_detected!("sse2") {
            unsafe { Simplex::gen_3d_impl::<Sse2, N>(start_x, start_y, start_z, freq, seed, out); }
        } else {
            unsafe { Simplex::gen_3d_impl::<Scalar, N>(start_x, start_y, start_z, freq, seed, out); }
        }
    }

    unsafe fn gen_3d_impl<S: Simd, const N: u32>(start_x: i32, start_y: i32, start_z: i32, freq: f32, seed: i32, out: &mut [f32]) {
        let seed = S::set1_epi32(seed);
        
        /* let mut min = S::set1_ps(f32::MAX);
        let mut max = S::set1_ps(f32::MIN); */
        
        let mut x_idx = S::set1_epi32(start_x as i32); 
        let mut y_idx = S::set1_epi32(start_y as i32);
        let mut z_idx = S::set1_epi32(start_z as i32);

        let freq_v = S::set1_ps(freq);
        let size_v = S::set1_epi32(N as i32);

        let x_max = x_idx + S::set1_epi32(N as i32 - 1);
        let y_max = y_idx + S::set1_epi32(N as i32 - 1);

        x_idx += incremented_i32::<S>();

        let total_values = N * N * N;
        let mut index = 0;
        while index < total_values as usize - S::VI32_WIDTH {
            let x_pos = S::cvtepi32_ps(x_idx) * freq_v;
            let y_pos = S::cvtepi32_ps(y_idx) * freq_v;
            let z_pos = S::cvtepi32_ps(z_idx) * freq_v;

            let gen = gen::<S>(seed, x_pos, y_pos, z_pos);
            S::storeu_ps(out.get_unchecked_mut(index as usize), gen);

            /* min = S::min_ps(min, gen);
            max = S::max_ps(max, gen); */

            index += S::VI32_WIDTH;
            x_idx += S::set1_epi32(S::VI32_WIDTH as i32);

            let x_reset = S::cmpgt_epi32(x_idx, x_max);
            y_idx -= x_reset;
            x_idx -= size_v & x_reset;

            let y_reset = S::cmpgt_epi32(y_idx, y_max);
            z_idx -= y_reset;
            y_idx -= size_v & y_reset;
        }

        let x_pos = S::cvtepi32_ps(x_idx) * freq_v;
        let y_pos = S::cvtepi32_ps(y_idx) * freq_v;
        let z_pos = S::cvtepi32_ps(z_idx) * freq_v;

        let gen = gen::<S>(seed, x_pos, y_pos, z_pos);

        /* let mut rmin = f32::MAX;
        let mut rmax = f32::MIN; */

        let remaining = total_values as usize - index;
        if remaining == S::VI32_WIDTH {
            S::storeu_ps(out.get_unchecked_mut(index as usize), gen);
            /* min = S::min_ps(min, gen);
            max = S::max_ps(max, gen); */
        } else {
            for j in 0..remaining {
                let n = gen[j as usize];
                *out.get_unchecked_mut(index as usize) = n;
                /* rmin = rmin.min(n);
                rmax = rmax.max(n); */
                index += 1;
            }
        }

        /* for i in 0..S::VI32_WIDTH {
            rmin = rmin.min(min[i]);
            rmax = rmax.max(max[i]);
        } */

        /* (rmin, rmax) */
    }
} 

unsafe fn incremented_i32<S: Simd>() -> S::Vi32 {
    let vals : [i32;8] = [0, 1, 2, 3, 4, 5, 6, 7];
    S::loadu_epi32(vals.get_unchecked(0))
}

unsafe fn gen<S: Simd>( seed: S::Vi32, x: S::Vf32, y: S::Vf32, z: S::Vf32) -> S::Vf32 {
    const F3 : f32 = 1.0 / 3.0;
    const G3 : f32 = 1.0 / 2.0;

    let s = S::set1_ps(F3) * (x + y + z);
    let x = x + s;
    let y = y + s;
    let z = z + s;

    // Full parts
    let mut x0 = S::fast_floor_ps(x);
    let mut y0 = S::fast_floor_ps(y);
    let mut z0 = S::fast_floor_ps(z);

    // Fractional parts
    let xi = x - x0;
    let yi = y - y0;
    let zi = z - z0;

    let i = S::cvtps_epi32( x0 ) * S::set1_epi32( 501125321 );
    let j = S::cvtps_epi32( y0 ) * S::set1_epi32( 1136930381 );
    let k = S::cvtps_epi32( z0 ) * S::set1_epi32( 1720413743 );

    let x_ge_y = S::castps_epi32(S::cmpge_ps(xi, yi));
    let y_ge_z = S::castps_epi32(S::cmpge_ps(yi,zi));
    let x_ge_z = S::castps_epi32(S::cmpge_ps(xi, zi));

    let g = S::set1_ps(G3) * (xi + yi + zi);
    x0 = xi - g;
    y0 = yi - g;
    z0 = zi - g;

    let i1 = x_ge_y & x_ge_z;
    let j1 = S::andnot_epi32( x_ge_y, y_ge_z  );
    let k1 = S::andnot_epi32( y_ge_z, !x_ge_z  );

    let i2 = x_ge_y | x_ge_z;
    let j2 = !x_ge_y | y_ge_z;
    let k2 = x_ge_z & y_ge_z; //NMasked

    let x1 = masked_sub::<S>( x0, S::set1_ps( 1.0 ), i1 ) + S::set1_ps(G3);
    let y1 = masked_sub::<S>( y0, S::set1_ps( 1.0 ), j1 ) + S::set1_ps(G3);
    let z1 = masked_sub::<S>( z0, S::set1_ps( 1.0 ), k1 ) + S::set1_ps(G3);
    let x2 = masked_sub::<S>( x0, S::set1_ps( 1.0 ), i2 ) + S::set1_ps(G3* 2.0);
    let y2 = masked_sub::<S>( y0, S::set1_ps( 1.0 ), j2 ) + S::set1_ps(G3* 2.0);
    let z2 = nmasked_sub::<S>( z0, S::set1_ps( 1.0 ), k2 ) + S::set1_ps(G3* 2.0);
    let x3 = x0 + S::set1_ps(G3 * 3.0 - 1.0);
    let y3 = y0 + S::set1_ps(G3 * 3.0 - 1.0);
    let z3 = z0 + S::set1_ps(G3 * 3.0 - 1.0);

    let mut t0 = S::fnmadd_ps( x0, x0, S::fnmadd_ps( y0, y0, S::fnmadd_ps( z0, z0, S::set1_ps( 0.6) ) ) );
    let mut t1 = S::fnmadd_ps( x1, x1, S::fnmadd_ps( y1, y1, S::fnmadd_ps( z1, z1, S::set1_ps( 0.6 ) ) ) );
    let mut t2 = S::fnmadd_ps( x2, x2, S::fnmadd_ps( y2, y2, S::fnmadd_ps( z2, z2, S::set1_ps( 0.6 ) ) ) );
    let mut t3 = S::fnmadd_ps( x3, x3, S::fnmadd_ps( y3, y3, S::fnmadd_ps( z3, z3, S::set1_ps( 0.6 ) ) ) );

    t0 = S::max_ps( t0, S::set1_ps( 0.0 ) );
    t1 = S::max_ps( t1, S::set1_ps( 0.0 ) );
    t2 = S::max_ps( t2, S::set1_ps( 0.0 ) );
    t3 = S::max_ps( t3, S::set1_ps( 0.0 ) );

    t0 *= t0; t0 *= t0;
    t1 *= t1; t1 *= t1;
    t2 *= t2; t2 *= t2;
    t3 *= t3; t3 *= t3;             

    let n0 = get_gradient_dot::<S>( hash_3_primes::<S>( seed, i, j, k), x0, y0, z0);
    let n1 = get_gradient_dot::<S>( hash_3_primes::<S>( seed, masked_add_i32::<S>( i, S::set1_epi32( 501125321 ), i1 ), masked_add_i32::<S>( j, S::set1_epi32( 1136930381 ), j1 ), masked_add_i32::<S>( k, S::set1_epi32( 1720413743 ), k1 ) ), x1, y1, z1 );
    let n2 = get_gradient_dot::<S>( hash_3_primes::<S>( seed, masked_add_i32::<S>( i, S::set1_epi32( 501125321 ), i2 ), masked_add_i32::<S>( j, S::set1_epi32( 1136930381 ), j2 ), nmasked_add_i32::<S>( k, S::set1_epi32( 1720413743 ), k2 ) ), x2, y2, z2 );
    let n3 = get_gradient_dot::<S>( hash_3_primes::<S>( seed, i + S::set1_epi32( 501125321 ), j + S::set1_epi32( 1136930381 ), k + S::set1_epi32( 1720413743 ) ), x3, y3, z3 );

    S::set1_ps(32.694_283) * S::fmadd_ps( n0, t0, S::fmadd_ps( n1, t1, S::fmadd_ps( n2, t2, n3 * t3 )))
}

#[inline(always)]
unsafe fn masked_sub<S: Simd>(a: S::Vf32, b: S::Vf32, m: S::Vi32) -> S::Vf32 {
    a - (b & S::castepi32_ps(m))
}

#[inline(always)]
unsafe fn nmasked_sub<S: Simd>(a: S::Vf32, b: S::Vf32, m: S::Vi32) -> S::Vf32 {
    a -S::andnot_ps(S::castepi32_ps(m), b)
}

#[inline(always)]
unsafe fn masked_add_i32<S: Simd>(a: S::Vi32, b: S::Vi32, m: S::Vi32) -> S::Vi32 {
    a + (b & m)
}

#[inline(always)]
unsafe fn nmasked_add_i32<S: Simd>(a: S::Vi32, b: S::Vi32, m: S::Vi32) -> S::Vi32 {
    a + S::andnot_epi32(m, b)
}

#[inline(always)]
unsafe fn get_gradient_dot<S: Simd>( hash: S::Vi32, f_x: S::Vf32,  f_y: S::Vf32,  f_z: S::Vf32 ) -> S::Vf32 {
    let hasha13 = hash & S::set1_epi32( 13 );

    let u = select_ps::<S>( S::cmpgt_epi32(S::set1_epi32( 8 ), hasha13), f_x, f_y );

    let v = select_ps::<S>( S::cmpeq_epi32(hasha13, S::set1_epi32( 12 )), f_x, f_z );
    let v = select_ps::<S>( S::cmpgt_epi32(S::set1_epi32( 2 ), hasha13), f_y, v );

    let h1 = S::castepi32_ps( hash << 31 );
    let h2 = S::castepi32_ps( (hash & S::set1_epi32( 2 )) << 30 );
    ( u ^ h1 ) + ( v ^ h2 )
}

#[inline(always)]
unsafe fn select_ps<S: Simd>( m: S::Vi32, a: S::Vf32, b: S::Vf32 ) -> S::Vf32 {
    S::blendv_ps( b, a, S::castepi32_ps( m ) )
}

#[inline(always)]
unsafe fn hash_3_primes<S: Simd>( seed: S::Vi32, a: S::Vi32, b: S::Vi32, c: S::Vi32) -> S::Vi32 {
    let mut hash = seed ^ a ^ b ^ c;
    hash *= S::set1_epi32( 0x27d4eb2d );
    (hash >> 15) ^ hash
}