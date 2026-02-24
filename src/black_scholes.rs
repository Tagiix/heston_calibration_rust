//! Modèle de Black-Scholes.
//!
//! Utilisé ici pour deux rôles :
//!   1. Pricer de référence (comparaison avec Heston).
//!   2. Convertisseur prix ↔ volatilité implicite.
//!
//! Formule Black-Scholes pour un call européen :
//!
//!   C = S·N(d₁) − K·e^{−rτ}·N(d₂)
//!
//!   d₁ = [ ln(S/K) + (r + σ²/2)·τ ] / (σ·√τ)
//!   d₂ = d₁ − σ·√τ
//!
//! où N(·) est la fonction de répartition de la loi normale standard.
// ---------------------------------------------------------------------------
// Densité et CDF normales
// ---------------------------------------------------------------------------
///
/// Densité de la loi normale standard : n(x) = (1/√(2π))·exp(−x²/2)
pub fn normal_pdf(x: f64) -> f64 {
    (-0.5 * x * x).exp() / (2.0 * std::f64::consts::PI).sqrt()
}

/// Fonction de répartition de la loi normale standard N(x) = P(Z ≤ x).
///
/// # Méthode : approximation rationnelle (Abramowitz & Stegun, 26.2.17)
///
/// Pour x ≥ 0 :
///
///   t = 1 / (1 + 0.2316419 · x)
///
///   N(x) ≈ 1 − n(x) · (b₁·t + b₂·t² + b₃·t³ + b₄·t⁴ + b₅·t⁵)
///
/// avec les coefficients :
///   b₁ =  0.319381530
///   b₂ = −0.356563782
///   b₃ =  1.781477937
///   b₄ = −1.821255978
///   b₅ =  1.330274429
///
/// Pour x < 0 : utiliser la symétrie  N(x) = 1 − N(−x)
///
/// Erreur maximale : ≤ 7.5 × 10⁻⁸
pub fn normal_cdf(x: f64) -> f64 {
    // Symétrie : N(-x) = 1 - N(x)
    if x < 0.0 {
        return 1.0 - normal_cdf(-x);
    }

    // Abramowitz & Stegun 26.2.17
    let t = 1.0 / (1.0 + 0.2316419 * x);

    // Évaluation du polynôme par la méthode de Horner :
    //   p(t) = t·(b₁ + t·(b₂ + t·(b₃ + t·(b₄ + t·b₅))))
    let poly = t
        * (0.319381530
            + t * (-0.356563782 + t * (1.781477937 + t * (-1.821255978 + t * 1.330274429))));

    1.0 - normal_pdf(x) * poly
}

// ---------------------------------------------------------------------------
// Pricing
// ---------------------------------------------------------------------------

/// Prix d'un call européen selon Black-Scholes.
///
/// # Arguments
/// - `spot`   : S₀, prix courant de l'actif
/// - `strike` : K, prix d'exercice
/// - `rate`   : r, taux sans risque (continu, annuel)
/// - `vol`    : σ, volatilité (annuelle)
/// - `tau`    : τ = T − t, durée jusqu'à maturité (en années)
///
/// # Formule
///
///   d₁ = [ ln(S/K) + (r + σ²/2)·τ ] / (σ·√τ)
///   d₂ = d₁ − σ·√τ
///   C  = S·N(d₁) − K·e^{−rτ}·N(d₂)
pub fn price_call(spot: f64, strike: f64, rate: f64, vol: f64, tau: f64) -> f64 {
    let d1 = ((spot / strike).ln() + tau * (rate + (vol * vol / 2.0))) / (vol * tau.sqrt());
    let d2 = d1 - vol * tau.sqrt();
    spot * normal_cdf(d1) - strike * (-rate * tau).exp() * normal_cdf(d2)
}

/// Prix d'un put européen par la parité call-put.
///
///   Put = Call − Spot + Strike·e^{−rτ}
pub fn price_put(spot: f64, strike: f64, rate: f64, vol: f64, tau: f64) -> f64 {
    price_call(spot, strike, rate, vol, tau) - spot + strike * (-rate * tau).exp()
}

// ---------------------------------------------------------------------------
// Volatilité implicite
// ---------------------------------------------------------------------------

/// Calcule la volatilité implicite d'un call européen par bisection.
///
/// Étant donné un prix de marché `price`, cherche σ* tel que :
///
///   BS_call(spot, strike, rate, σ*, tau) = price
///
/// Retourne `None` si aucune solution n'est trouvée (prix hors des bornes).
///
/// # Méthode : bisection sur [σ_low, σ_high]
///
/// L'idée : le prix BS est une fonction croissante et continue de σ.
/// On cherche donc le zéro de  f(σ) = BS(σ) − price.
///
/// Algorithme :
///   1. Définir [σ_low = 1e-6, σ_high = 5.0] comme bornes initiales.
///   2. Vérifier que price ∈ [BS(σ_low), BS(σ_high)] — sinon, retourner None.
///   3. Boucler jusqu'à convergence (|σ_high − σ_low| < tolérance) :
///        - σ_mid = (σ_low + σ_high) / 2
///        - Si BS(σ_mid) > price : σ_high = σ_mid
///        - Sinon              : σ_low  = σ_mid
///   4. Retourner Some((σ_low + σ_high) / 2)
///
/// Tolérance suggérée : 1e-7. Max itérations : 100.
pub fn implied_volatility(price: f64, spot: f64, strike: f64, rate: f64, tau: f64) -> Option<f64> {
    let mut sigma_low = 1e-6;
    let mut sigma_high = 5.0;
    let max_iter = 100;
    let tolerance = 1e-7;

    if price > price_call(spot, strike, rate, sigma_high, tau)
        || price < price_call(spot, strike, rate, sigma_low, tau)
    {
        return None;
    }

    for _ in 0..max_iter {
        if (sigma_high - sigma_low) < tolerance {
            return Some((sigma_high + sigma_low) / 2.0);
        } else {
            let sigma_mid = (sigma_high + sigma_low) / 2.0;
            if price_call(spot, strike, rate, sigma_mid, tau) > price {
                sigma_high = sigma_mid;
            } else {
                sigma_low = sigma_mid;
            }
        }
    }
    None
}
