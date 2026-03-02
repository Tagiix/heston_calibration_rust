/// Tests d'intégration — valident chaque module indépendamment.
///
/// Lance avec : `cargo test`
///
/// Ces tests te permettent de vérifier chaque implémentation au fur et à mesure.
/// Commence par les tests BS (les plus simples), puis passe aux tests Heston.
use approx::assert_abs_diff_eq;

use heston_calibration::black_scholes;
use heston_calibration::heston::{self, HestonParams};
use heston_calibration::fourier;

// ============================================================================
// Tests Black-Scholes
// ============================================================================

#[test]
fn test_normal_cdf_at_zero() {
    // N(0) = 0.5 exactement
    assert_abs_diff_eq!(black_scholes::normal_cdf(0.0), 0.5, epsilon = 1e-7);
}

#[test]
fn test_normal_cdf_positive() {
    // N(1.96) ≈ 0.975 (intervalle de confiance à 95%)
    assert_abs_diff_eq!(black_scholes::normal_cdf(1.96), 0.97500, epsilon = 1e-4);
}

#[test]
fn test_normal_cdf_negative() {
    // N(-1.0) = 1 - N(1.0) ≈ 0.1587
    let n1 = black_scholes::normal_cdf(1.0);
    let n_neg1 = black_scholes::normal_cdf(-1.0);
    assert_abs_diff_eq!(n1 + n_neg1, 1.0, epsilon = 1e-7);
}

#[test]
fn test_normal_cdf_symmetry() {
    // N(x) + N(-x) = 1 pour tout x
    for x in [0.5, 1.0, 1.5, 2.0, 3.0] {
        assert_abs_diff_eq!(
            black_scholes::normal_cdf(x) + black_scholes::normal_cdf(-x),
            1.0,
            epsilon = 1e-6
        );
    }
}

#[test]
fn test_bs_call_known_value() {
    // Prix de référence calculé indépendamment
    // Call(S=100, K=100, r=5%, σ=20%, τ=1y) ≈ 10.4506
    let price = black_scholes::price_call(100.0, 100.0, 0.05, 0.20, 1.0);
    assert_abs_diff_eq!(price, 10.4506, epsilon = 1e-3);
}

#[test]
fn test_bs_call_otm() {
    // Un call très OTM a une valeur proche de zéro
    let price = black_scholes::price_call(100.0, 200.0, 0.05, 0.20, 0.25);
    assert!(price >= 0.0);
    assert!(price < 1e-5);
}

#[test]
fn test_bs_call_put_parity() {
    // Parité call-put : C - P = S - K·e^{-rτ}
    let (spot, strike, rate, vol, tau) = (100.0, 95.0, 0.05, 0.25, 0.5);
    let call = black_scholes::price_call(spot, strike, rate, vol, tau);
    let put  = black_scholes::price_put(spot, strike, rate, vol, tau);
    let parity = spot - strike * (-rate * tau).exp();
    assert_abs_diff_eq!(call - put, parity, epsilon = 1e-8);
}

#[test]
fn test_implied_vol_roundtrip() {
    // Calculer un prix, inverser en IV, doit retrouver σ = 0.20
    let (spot, strike, rate, vol, tau) = (100.0, 100.0, 0.05, 0.20, 1.0);
    let price = black_scholes::price_call(spot, strike, rate, vol, tau);
    let iv = black_scholes::implied_volatility(price, spot, strike, rate, tau)
        .expect("La vol implicite doit exister pour ce prix");
    assert_abs_diff_eq!(iv, vol, epsilon = 1e-5);
}

#[test]
fn test_implied_vol_roundtrip_various() {
    // Test sur plusieurs strikes et maturités
    let spot = 100.0;
    let rate = 0.05;
    for &sigma in &[0.10, 0.20, 0.30, 0.50] {
        for &strike in &[80.0, 100.0, 120.0] {
            for &tau in &[0.25, 1.0] {
                let price = black_scholes::price_call(spot, strike, rate, sigma, tau);
                let iv = black_scholes::implied_volatility(price, spot, strike, rate, tau)
                    .unwrap_or(f64::NAN);
                assert_abs_diff_eq!(iv, sigma, epsilon = 1e-4);
            }
        }
    }
}

// ============================================================================
// Tests de la fonction caractéristique de Heston
// ============================================================================

#[test]
fn test_characteristic_function_at_zero() {
    // φ(0) = E[e^0] = 1 (par définition d'une fonction caractéristique)
    use num_complex::Complex64;
    let params = HestonParams::new(2.0, 0.04, 0.3, -0.7, 0.04);
    let phi0 = heston::characteristic_function(Complex64::new(0.0, 0.0), &params, 1.0);
    assert_abs_diff_eq!(phi0.re, 1.0, epsilon = 1e-10);
    assert_abs_diff_eq!(phi0.im, 0.0, epsilon = 1e-10);
}

#[test]
fn test_characteristic_function_modulus() {
    // |φ(u)| ≤ 1 pour u réel (propriété de toute fonction caractéristique de proba)
    use num_complex::Complex64;
    let params = HestonParams::new(2.0, 0.04, 0.3, -0.7, 0.04);
    for v in [0.5, 1.0, 2.0, 5.0, 10.0] {
        let phi = heston::characteristic_function(Complex64::new(v, 0.0), &params, 1.0);
        assert!(phi.norm() <= 1.0 + 1e-10, "φ({}) a un module > 1", v);
    }
}

// ============================================================================
// Tests du pricer Heston (Fourier)
// ============================================================================

#[test]
fn test_heston_call_positive() {
    // Tout prix d'option doit être positif
    let params = HestonParams::new(2.0, 0.04, 0.3, -0.7, 0.04);
    for &strike in &[80.0, 100.0, 120.0] {
        let price = fourier::price_call_heston(&params, 100.0, strike, 0.05, 1.0);
        assert!(price > 0.0, "Le prix doit être positif pour K={}", strike);
    }
}

#[test]
fn test_heston_no_vol_of_vol_approx_bs() {
    // Quand ξ → 0, le modèle de Heston se réduit à Black-Scholes avec σ = √v₀.
    // On teste avec ξ petit mais non nul.
    let sigma = 0.20_f64;
    let params = HestonParams::new(
        100.0,       // κ très grand → v revient très vite à θ
        sigma * sigma, // θ = σ²
        0.001,       // ξ très petit (quasi-déterministe)
        0.0,         // ρ = 0
        sigma * sigma, // v₀ = σ²
    );
    let heston_price = fourier::price_call_heston(&params, 100.0, 100.0, 0.05, 1.0);
    let bs_price     = black_scholes::price_call(100.0, 100.0, 0.05, sigma, 1.0);

    // Tolérance large car ξ n'est pas exactement 0
    assert_abs_diff_eq!(heston_price, bs_price, epsilon = 0.05);
}

#[test]
fn test_heston_call_monotone_in_strike() {
    // Le prix d'un call est une fonction décroissante du strike
    let params = HestonParams::new(2.0, 0.04, 0.3, -0.7, 0.04);
    let strikes = [80.0, 90.0, 100.0, 110.0, 120.0];
    let prices: Vec<f64> = strikes.iter()
        .map(|&k| fourier::price_call_heston(&params, 100.0, k, 0.05, 1.0))
        .collect();

    for window in prices.windows(2) {
        assert!(window[0] > window[1],
            "Le prix doit décroître avec le strike : {} <= {}",
            window[0], window[1]
        );
    }
}
