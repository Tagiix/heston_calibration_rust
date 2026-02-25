/// Modèle de Heston (1993) : volatilité stochastique.
///
/// Dynamique sous la mesure risque-neutre Q :
///
///   dS = r·S·dt + √v·S·dW^S
///   dv = κ(θ - v)·dt + ξ·√v·dW^v
///
/// avec  d⟨W^S, W^v⟩ = ρ·dt
///
/// Cinq paramètres à calibrer : κ, θ, ξ, ρ, v₀.
use num_complex::{Complex, Complex64};

// ---------------------------------------------------------------------------
// Paramètres
// ---------------------------------------------------------------------------

/// Paramètres du modèle de Heston.
#[derive(Debug, Clone, PartialEq)]
pub struct HestonParams {
    /// Vitesse de retour à la moyenne (κ > 0)
    pub kappa: f64,
    /// Variance long-terme (θ > 0)
    pub theta: f64,
    /// Volatilité de la volatilité (ξ > 0)
    pub xi: f64,
    /// Corrélation entre l'actif et la variance (-1 < ρ < 1)
    pub rho: f64,
    /// Variance initiale (v₀ > 0)
    pub v0: f64,
}

impl HestonParams {
    pub fn new(kappa: f64, theta: f64, xi: f64, rho: f64, v0: f64) -> Self {
        HestonParams {
            kappa,
            theta,
            xi,
            rho,
            v0,
        }
    }

    /// Condition de Feller : 2κθ > ξ²
    ///
    /// Quand cette condition est satisfaite, le processus de variance v_t
    /// reste strictement positif (ne touche jamais zéro).
    pub fn feller_condition(&self) -> bool {
        2.0 * self.kappa * self.theta > self.xi * self.xi
    }

    /// Vérifie que tous les paramètres sont dans des plages valides.
    pub fn is_valid(&self) -> bool {
        self.kappa > 0.0
            && self.theta > 0.0
            && self.xi > 0.0
            && self.rho > -1.0
            && self.rho < 1.0
            && self.v0 > 0.0
    }
}

// ---------------------------------------------------------------------------
// Fonction caractéristique
// ---------------------------------------------------------------------------

/// Calcule la fonction caractéristique de ln(S_T / F) sous la mesure Q.
///
/// Définition :
///
///   φ(u) = E^Q[ e^{iu · ln(S_T/F)} ]
///
/// où F = S₀ · e^{rτ} est le prix forward.
///
/// Note : φ ne dépend *pas* de S₀ ni de r directement — ceux-ci sont déjà
/// absorbés dans la log-moneyness ln(K/F) lors du pricing.
///
/// # Formule de Heston (forme fermée)
///
/// Posons :
///
///   β  = κ - ρ·ξ·i·u
///   d  = sqrt( β² + ξ²·(i·u + u²) )    (racine complexe, branche principale)
///   g  = (β - d) / (β + d)
///
/// Alors :
///
///   C(u, τ) = (κ·θ / ξ²) · [ (β - d)·τ  -  2·ln( (1 - g·e^{-dτ}) / (1 - g) ) ]
///
///   D(u, τ) = (β - d) / ξ²  ·  (1 - e^{-dτ}) / (1 - g·e^{-dτ})
///
///   φ(u)    = exp( C(u, τ)  +  D(u, τ) · v₀ )
/// # Arguments
///
/// - `u`   : fréquence complexe (typiquement réelle dans l'intégrale de pricing)
/// - `params` : paramètres du modèle
/// - `tau` : τ = T − t, durée jusqu'à maturité (en années)
pub fn characteristic_function(u: Complex64, params: &HestonParams, tau: f64) -> Complex64 {
    let i = Complex64::i();
    let beta: Complex64 = params.kappa - params.rho * params.xi * i * u;
    let d = (beta * beta + params.xi * params.xi * (i * u + u * u)).sqrt();
    let g = (beta - d) / (beta + d);
    let exp_neg_d_tau = (-d * tau).exp();
    //   C(u, τ) = (κ·θ / ξ²) · [ (β - d)·τ  -  2·ln( (1 - g·e^{-dτ}) / (1 - g) ) ]
    let c = (params.kappa * params.theta / (params.xi * params.xi))
        * (((beta - d) * tau) - 2.0 * ((1.0 - g * exp_neg_d_tau) / (1.0 - g)).ln());
    //   D(u, τ) = (β - d) / ξ²  ·  (1 - e^{-dτ}) / (1 - g·e^{-dτ})
    let d = ((beta - d) / (params.xi * params.xi))
        * ((1.0 - exp_neg_d_tau) / (1.0 - g * exp_neg_d_tau));
    //   φ(u)    = exp( C(u, τ)  +  D(u, τ) · v₀ )
    (c + d * params.v0).exp()
}
