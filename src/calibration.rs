use core::f64;

use argmin::core::{CostFunction, Error as ArgminError, Executor};
use argmin::solver::neldermead::NelderMead;

use crate::black_scholes::implied_volatility;
use crate::fourier::price_call_heston;
/// Calibration du modèle de Heston sur une surface de volatilité implicite.
///
/// Objectif : trouver les paramètres θ* = (κ, θ, ξ, ρ, v₀) qui minimisent
/// l'erreur quadratique entre les volatilités implicites du modèle et du marché.
use crate::heston::HestonParams;

// ---------------------------------------------------------------------------
// Structure de données marché
// ---------------------------------------------------------------------------

/// Une cotation de marché : strike, maturité et volatilité implicite observée.
#[derive(Debug, Clone)]
pub struct MarketQuote {
    pub strike: f64,
    pub tau: f64,
    pub implied_vol: f64,
}

// ---------------------------------------------------------------------------
// Fonction objectif
// ---------------------------------------------------------------------------

/// Calcule la RMSE entre les volatilités implicites du modèle et du marché.
///
/// Pour chaque cotation :
///   1. On calcule le prix de l'option avec le modèle de Heston.
///   2. On inverse ce prix en volatilité implicite (Black-Scholes).
///   3. On calcule l'erreur carrée par rapport à la vol implicite de marché.
///
/// Résultat :
///
///   RMSE = sqrt( (1/N) · Σᵢ (IV_modèle(i) − IV_marché(i))² )
///
/// Si la vol implicite ne peut pas être calculée pour un point (prix hors
/// des bornes), ignorer ce point (ou lui attribuer une pénalité forte ???).
///
/// # Arguments
/// - `params` : paramètres Heston à évaluer
/// - `quotes` : cotations de marché
/// - `spot`   : prix de l'actif sous-jacent
/// - `rate`   : taux sans risque
pub fn objective(params: &HestonParams, quotes: &[MarketQuote], spot: f64, rate: f64) -> f64 {
    let mut sse = 0.0_f64;
    let mut count = 0_usize;

    for quote in quotes {
        {
            let model_price = price_call_heston(params, spot, quote.strike, rate, quote.tau);
            if let Some(model_iv) =
                implied_volatility(model_price, spot, quote.strike, rate, quote.tau)
            {
                {
                    let error = model_iv - quote.implied_vol;
                    sse += error * error;
                    count += 1;
                }
            }
        }
    }

    if count == 0 {
        {
            return f64::INFINITY;
        }
    }
    (sse / count as f64).sqrt()
}

// ---------------------------------------------------------------------------
// Calibration
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Helpers pour la conversion vecteur ↔ HestonParams
// ---------------------------------------------------------------------------

fn params_to_vec(p: &HestonParams) -> Vec<f64> {
    vec![p.kappa, p.theta, p.xi, p.rho, p.v0]
}

fn vec_to_params(v: &[f64]) -> HestonParams {
    HestonParams::new(
        v[0].clamp(0.1, 10.0),
        v[1].clamp(0.001, 1.0),
        v[2].clamp(0.01, 2.0),
        v[3].clamp(-0.99, 0.99),
        v[4].clamp(0.001, 1.0),
    )
}

// ---------------------------------------------------------------------------
// Problème d'optimisation pour argmin
// ---------------------------------------------------------------------------

struct HestonObjective<'a> {
    quotes: &'a [MarketQuote],
    spot: f64,
    rate: f64,
}

impl CostFunction for HestonObjective<'_> {
    type Param = Vec<f64>;
    type Output = f64;

    fn cost(&self, p: &Vec<f64>) -> Result<f64, ArgminError> {
        let params = vec_to_params(p);
        Ok(objective(&params, self.quotes, self.spot, self.rate))
    }
}

// ---------------------------------------------------------------------------
// Calibration
// ---------------------------------------------------------------------------

/// Calibre le modèle de Heston sur les cotations de marché.
///
/// Retourne les paramètres qui minimisent `objective`.
///
/// ## Phase 1 — Recherche sur grille grossière
///
/// ## Phase 2 — Raffinement local via Nelder-Mead (argmin)
///
/// Le simplexe initial est construit autour du meilleur point de la grille
/// en perturbant chaque dimension de ±0.1.
/// Nombre de candidats retenus depuis la grille pour le multi-start.
const TOP_K: usize = 5;

/// Construit le simplexe initial autour d'un point de départ :
/// le point lui-même + N sommets, chacun perturbé d'un pas sur une dimension.
fn make_simplex(init: &[f64], step: f64) -> Vec<Vec<f64>> {
    let mut simplex = vec![init.to_vec()];
    for i in 0..init.len() {
        let mut vertex = init.to_vec();
        vertex[i] += step;
        simplex.push(vertex);
    }
    simplex
}

/// Lance NelderMead depuis un point de départ et retourne (coût, params).
fn run_nelder_mead(
    init: Vec<f64>,
    quotes: &[MarketQuote],
    spot: f64,
    rate: f64,
    max_iters: u64,
) -> (f64, Vec<f64>) {
    let simplex = make_simplex(&init, 0.1);

    let solver = NelderMead::new(simplex)
        .with_sd_tolerance(1e-7)
        .expect("sd_tolerance valide");

    let result = Executor::new(HestonObjective { quotes, spot, rate }, solver)
        .configure(|state| state.max_iters(max_iters))
        .run()
        .expect("NelderMead a convergé");

    let best_vec = result
        .state()
        .best_param
        .clone()
        .expect("argmin: aucun paramètre optimal trouvé");
    let best_cost = result.state().best_cost;

    (best_cost, best_vec)
}

pub fn calibrate(
    quotes: &[MarketQuote],
    spot: f64,
    rate: f64,
    _initial_params: HestonParams,
    fast: bool,
) -> HestonParams {
    // Paramètres de la grille et de l'optimisation selon le mode
    let (kappas, thetas, xis, rhos, v0s, top_k, max_iters): (
        &[f64], &[f64], &[f64], &[f64], &[f64], usize, u64,
    ) = if fast {
        // Mode rapide : grille 2×2×2×2×2 = 32 points, 1 start, 100 itérations
        (
            &[1.0_f64, 3.0],
            &[0.03_f64, 0.06],
            &[0.2_f64, 0.6],
            &[-0.7_f64, -0.3],
            &[0.03_f64, 0.06],
            1,
            100,
        )
    } else {
        // Mode précis : grille 4×3×3×3×3 = 324 points, 5 starts, 400 itérations
        (
            &[0.5_f64, 1.0, 2.0, 4.0],
            &[0.02_f64, 0.04, 0.06],
            &[0.1_f64, 0.3, 0.5],
            &[-0.8_f64, -0.5, -0.2],
            &[0.02_f64, 0.04, 0.06],
            TOP_K,
            400,
        )
    };

    // -----------------------------------------------------------------------
    // Phase 1 — Grille grossière : collecter tous les candidats
    // -----------------------------------------------------------------------
    let mut candidates: Vec<(f64, HestonParams)> = Vec::new();

    for &kappa in kappas {
        for &theta in thetas {
            for &xi in xis {
                for &rho in rhos {
                    for &v0 in v0s {
                        let params = HestonParams::new(kappa, theta, xi, rho, v0);
                        let obj = objective(&params, quotes, spot, rate);
                        candidates.push((obj, params));
                    }
                }
            }
        }
    }

    // Trier par coût croissant, garder les top_k meilleurs points de départ
    candidates.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Greater));
    candidates.truncate(top_k);

    // -----------------------------------------------------------------------
    // Phase 2 — Multi-start Nelder-Mead : un run par candidat
    // -----------------------------------------------------------------------
    let (_, best_vec) = candidates
        .into_iter()
        .map(|(_, params)| run_nelder_mead(params_to_vec(&params), quotes, spot, rate, max_iters))
        .min_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Greater))
        .expect("au moins un candidat");

    vec_to_params(&best_vec)
}
