use std::arch::x86_64::*;

#[cfg(target_arch = "x86_64")]
pub mod vector_ops {
    use super::*;
    use crate::common::config;
    
    #[target_feature(enable = "avx512f")]
    pub unsafe fn simd_l2_norm(data: &[f32]) -> f32 {
        let mut sum = _mm512_setzero_ps();
        
        for chunk in data.chunks_exact(16) {
            let v = _mm512_loadu_ps(chunk.as_ptr());
            sum = _mm512_fmadd_ps(v, v, sum);
        }
        
        _mm512_reduce_add_ps(sum).sqrt()
    }
    
    #[target_feature(enable = "avx512f")]
    pub unsafe fn normalize_vector(data: &mut [f32]) {
        let norm = simd_l2_norm(data);
        if norm > 0.0 {
            for chunk in data.chunks_exact_mut(16) {
                let v = _mm512_loadu_ps(chunk.as_ptr());
                let normalized = _mm512_div_ps(v, _mm512_set1_ps(norm));
                _mm512_storeu_ps(chunk.as_mut_ptr(), normalized);
            }
        }
    }
    
    #[target_feature(enable = "avx512f")]
    pub unsafe fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
        assert_eq!(a.len(), b.len());
        let mut dot_product = _mm512_setzero_ps();
        
        for (chunk_a, chunk_b) in a.chunks_exact(16).zip(b.chunks_exact(16)) {
            let va = _mm512_loadu_ps(chunk_a.as_ptr());
            let vb = _mm512_loadu_ps(chunk_b.as_ptr());
            dot_product = _mm512_fmadd_ps(va, vb, dot_product);
        }
        
        _mm512_reduce_add_ps(dot_product)
    }
}
