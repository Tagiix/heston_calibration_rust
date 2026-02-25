use crate::heston::{characteristic_function, HestonParams};
/// Pricing d'options par intégration de Fourier (méthode de Carr-Madan 1999).
///
/// Idée : la transformée de Fourier du prix d'un call amorti peut s'exprimer
/// analytiquement via la fonction caractéristique du log-retour.
/// On récupère le prix par intégration numérique inverse.
///
/// Référence : Carr, P. & Madan, D. (1999). "Option valuation using the fast
/// Fourier transform." Journal of Computational Finance, 2(4), 61–73.
use num_complex::Complex64;

// ---------------------------------------------------------------------------
// Intégration numérique
// ---------------------------------------------------------------------------

/// Intégration numérique par la règle de Simpson composite.
///
/// Approxime ∫ₐᵇ f(x) dx avec n sous-intervalles (n doit être pair).
///
/// # Formule
///
///   h = (b − a) / n
///
///   ∫ ≈ (h/3) · [ f(x₀) + 4f(x₁) + 2f(x₂) + 4f(x₃) + ⋯ + 4f(xₙ₋₁) + f(xₙ) ]
///
/// Le schéma des coefficients est : 1, 4, 2, 4, 2, …, 2, 4, 1.
///
/// # Arguments
/// - `f` : fonction à intégrer (closure prenant un f64, retournant un f64)
/// - `a`, `b` : bornes de l'intégrale
/// - `n` : nombre de sous-intervalles (sera arrondi au pair supérieur si impair)
pub fn simpson_integrate<F: Fn(f64) -> f64>(f: F, a: f64, b: f64, n: usize) -> f64 {
    let n_even = if n % 2 != 0 { n + 1 } else { n };
    let h: f64 = (b - a) / (n_even as f64);
    let mut sum: f64 = f(a) + f(b);
    for i in 1..n_even {
        let x: f64 = a + (i as f64) * h;
        let alpha: f64 = if i % 2 == 0 {
            {
                2.0
            }
        } else {
            {
                4.0
            }
        };
        sum += alpha * f(x)
    }
    sum * h / 3.0
}

// ---------------------------------------------------------------------------
// Pricing Heston par Carr-Madan
// ---------------------------------------------------------------------------

/// Prix d'un call européen dans le modèle de Heston, via Carr-Madan (1999).
///
/// # Formule de Carr-Madan
///
/// Soit :
///   F  = S₀ · e^{rτ}                      (prix forward)
///   k  = ln(K / F)                         (log-moneyness par rapport au forward)
///   α  = 1.5                               (facteur d'amortissement, α > 0)
///
/// On définit pour chaque fréquence v ∈ ℝ₊ :
///
///   φ̃(v) = φ(v − i(α+1))                  (CF évaluée en v − i(α+1))
///
///   ψ(v)  = φ̃(v) / [ α² + α − v² + iv(2α+1) ]
///
///   intégrand(v) = Re[ e^{−ivk} · ψ(v) ]
///
/// Le prix du call est alors :
///
///   C(K, T) = (S₀ · e^{−αk} / π) · ∫₀^{V_max} intégrand(v) dv
///
/// # Paramètres suggérés pour l'intégration
///   V_max     = 500.0   (borne supérieure de troncature)
///   n_points  = 1000    (nombre de points pour Simpson)
///
pub fn price_call_heston(
    params: &HestonParams,
    spot: f64,
    strike: f64,
    rate: f64,
    tau: f64,
) -> f64 {
    let alpha = 1.5_f64; // facteur d'amortissement
    let v_max = 500.0_f64; // borne supérieure de l'intégrale
    let n_points = 1000; // nombre de points (pair ✓)
    let forward = spot * (rate * tau).exp();
    let k = (strike / forward).ln(); // k = ln(K/F)

    // Pour chaque fréquence v (réelle), on évalue le noyau de Carr-Madan.
    // La closure capture `params`, `tau`, `k` et `alpha` depuis l'environnement.
    let integrand = |v: f64| -> f64 {
        // Argument complexe pour la fonction caractéristique : v − i(α+1)
        let u = Complex64::new(v, -(alpha + 1.0));

        // Évaluer la fonction caractéristique φ(u)
        let phi = characteristic_function(u, params, tau);

        // Dénominateur de Carr-Madan : α² + α − v² + iv(2α+1)
        let denom = Complex64::new(alpha * alpha + alpha - v * v, v * (2.0 * alpha + 1.0));

        // ψ(v) = φ(v−i(α+1)) / dénominateur
        let psi = phi / denom;

        // Facteur d'oscillation : e^{−ivk}
        let oscillation = Complex64::new(0.0, -v * k).exp();

        // Partie réelle de e^{−ivk} · ψ(v)
        (oscillation * psi).re
    };

    let integral = simpson_integrate(integrand, 0.0, v_max, n_points);

    // C = (S₀ · e^{−αk} / π) · intégrale
    spot * ((-alpha * k).exp() / std::f64::consts::PI) * integral
}
