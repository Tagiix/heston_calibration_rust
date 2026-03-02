/// Point d'entrée principal du moteur de calibration de Heston.
///
/// Ce fichier orchestre les 4 étapes du projet :
///   1. Validation du pricer Black-Scholes
///   2. Pricing Heston par transformée de Fourier (Carr-Madan)
///   3. Construction de la surface de vol implicite Heston
///   4. Calibration sur des données de marché
///
/// Lance chaque module au fur et à mesure que tu implémentes les fonctions.

mod black_scholes;
mod calibration;
mod fourier;
mod heston;
mod market_data;
mod utils;

// ---------------------------------------------------------------------------
// Parsing des arguments CLI
// ---------------------------------------------------------------------------

struct CliArgs {
    ticker:    Option<String>,
    realistic: bool,
}

fn parse_args() -> CliArgs {
    let args: Vec<String> = std::env::args().collect();

    if args.iter().any(|a| a == "--help" || a == "-h") {
        println!(
            "\
Usage: heston-calibration [OPTIONS]

Options:
  --ticker <TICKER>   Télécharge la chaîne d'options live depuis Yahoo Finance
                      (ex: --ticker SPY, --ticker AAPL)
                      Les données sont sauvegardées dans data/market_surface.csv
  --realistic         Utilise la surface statique SPX stylisée (45 points)
  (aucun flag)        Charge data/market_surface.csv ou génère une surface synthétique

Exemples:
  cargo run                       # Surface synthétique
  cargo run -- --realistic        # Surface réaliste style SPX
  cargo run -- --ticker SPY       # Données live Yahoo Finance
  cargo run -- --ticker AAPL      # Données live Apple
"
        );
        std::process::exit(0);
    }

    let ticker = args
        .windows(2)
        .find(|w| w[0] == "--ticker")
        .map(|w| w[1].clone());

    let realistic = args.iter().any(|a| a == "--realistic");

    CliArgs { ticker, realistic }
}

fn main() {
    let cli  = parse_args();
    let rate = 0.05_f64;
    // Spot par défaut pour les étapes 1-3 (toujours normalisé à 100)
    // — peut être écrasé à l'étape 4 par le prix live issu de Yahoo Finance.
    let spot = 100.0_f64;

    // =========================================================================
    // Étape 1 : Validation Black-Scholes
    // =========================================================================
    // À faire en premier — c'est la brique de base.
    // Implémenter : normal_cdf, price_call, implied_volatility

    println!("╔══════════════════════════════════════╗");
    println!("║   Étape 1 — Black-Scholes            ║");
    println!("╚══════════════════════════════════════╝");

    let bs_price = black_scholes::price_call(spot, 100.0, rate, 0.20, 1.0);
    println!("BS Call(S=100, K=100, r=5%, σ=20%, τ=1y) = {:.4}", bs_price);
    println!("Valeur attendue                           ≈ 10.4506");

    let iv = black_scholes::implied_volatility(bs_price, spot, 100.0, rate, 1.0);
    println!("Vol implicite retrouvée                   = {:.4?}", iv);
    println!("Valeur attendue                           ≈ Some(0.2000)");

    // =========================================================================
    // Étape 2 : Pricing Heston (Fourier)
    // =========================================================================
    // À faire en deuxième — dépend de la fonction caractéristique et de Simpson.
    // Implémenter : characteristic_function, simpson_integrate, price_call_heston

    println!();
    println!("╔══════════════════════════════════════╗");
    println!("║   Étape 2 — Pricing Heston (Fourier) ║");
    println!("╚══════════════════════════════════════╝");

    // Paramètres "vrais" utilisés pour générer les données synthétiques
    let true_params = heston::HestonParams::new(
        2.0,   // κ : retour à la moyenne rapide
        0.04,  // θ : variance long-terme = 20%² = 4%
        0.30,  // ξ : vol-of-vol
        -0.70, // ρ : corrélation négative (levier)
        0.04,  // v₀ : variance initiale = 20%²
    );

    println!("Condition de Feller satisfaite : {}", true_params.feller_condition());

    let heston_atm = fourier::price_call_heston(&true_params, spot, 100.0, rate, 1.0);
    println!("Heston ATM Call (K=100, τ=1y)   = {:.4}", heston_atm);
    println!("(doit être proche du prix BS avec σ ≈ 20%)");

    // =========================================================================
    // Étape 3 : Surface de volatilité implicite Heston
    // =========================================================================
    // Montre le sourire/skew généré par le modèle.

    println!();
    println!("╔══════════════════════════════════════╗");
    println!("║   Étape 3 — Surface IV Heston        ║");
    println!("╚══════════════════════════════════════╝");

    let strikes = [80.0, 90.0, 100.0, 110.0, 120.0];
    let taus    = [0.25, 0.5, 1.0];

    println!("{:<8} {:<8} {}", "Strike", "τ (an)", "IV (%)");
    println!("{}", "-".repeat(30));

    for &tau in &taus {
        for &k in &strikes {
            let price = fourier::price_call_heston(&true_params, spot, k, rate, tau);
            match black_scholes::implied_volatility(price, spot, k, rate, tau) {
                Some(iv) => println!("{:<8.0} {:<8.2} {:.2}%", k, tau, iv * 100.0),
                None     => println!("{:<8.0} {:<8.2} N/A", k, tau),
            }
        }
        println!();
    }

    // =========================================================================
    // Étape 4 : Calibration
    // =========================================================================
    // À faire en dernier — dépend de tout le reste.
    // Implémenter : objective, calibrate

    println!("╔══════════════════════════════════════╗");
    println!("║   Étape 4 — Calibration              ║");
    println!("╚══════════════════════════════════════╝");

    // ── Sélection de la source de données ────────────────────────────────────
    //
    //   1. --ticker TICKER  → Yahoo Finance (live)
    //   2. --realistic      → surface statique SPX stylisée
    //   3. data/market_surface.csv existe → chargement CSV
    //   4. (défaut)         → surface synthétique
    //
    let (spot, quotes): (f64, Vec<calibration::MarketQuote>) = if let Some(ref ticker) = cli.ticker {
        println!("Téléchargement des données Yahoo Finance pour {}…", ticker.to_uppercase());
        match market_data::fetch_yahoo(ticker) {
            Ok((live_spot, q)) => {
                // Sauvegarder pour réutilisation
                match utils::save_surface("data/market_surface.csv", &q) {
                    Ok(_)  => println!("  Sauvegardé dans data/market_surface.csv"),
                    Err(e) => eprintln!("  Avertissement : impossible de sauvegarder ({e})"),
                }
                (live_spot, q)
            }
            Err(e) => {
                eprintln!("Échec du téléchargement : {e}");
                eprintln!("Repli sur la surface réaliste statique.");
                (100.0, market_data::realistic_static_surface(100.0))
            }
        }
    } else if cli.realistic {
        println!("Utilisation de la surface réaliste statique (style SPX).");
        (100.0, market_data::realistic_static_surface(100.0))
    } else {
        match utils::load_surface("data/market_surface.csv") {
            Ok(q) => {
                println!("Surface chargée depuis CSV : {} cotations", q.len());
                (100.0, q)
            }
            Err(_) => {
                println!("CSV non trouvé — utilisation de la surface synthétique.");
                (100.0, utils::generate_synthetic_surface(100.0))
            }
        }
    };

    println!("\nSurface de marché ({} cotations) :", quotes.len());
    utils::print_vol_surface(&quotes);

    // Point de départ de l'optimisation (volontairement éloigné des vrais params)
    let initial = heston::HestonParams::new(1.0, 0.06, 0.50, -0.50, 0.06);

    println!("\nObjectif initial : {:.6}", calibration::objective(&initial, &quotes, spot, rate));

    let calibrated = calibration::calibrate(&quotes, spot, rate, initial);

    println!("\n╔══════════════════════════════════════╗");
    println!("║   Paramètres calibrés                ║");
    println!("╚══════════════════════════════════════╝");
    println!("  κ  = {:.4}   (vrai : 2.0000)", calibrated.kappa);
    println!("  θ  = {:.4}   (vrai : 0.0400)", calibrated.theta);
    println!("  ξ  = {:.4}   (vrai : 0.3000)", calibrated.xi);
    println!("  ρ  = {:.4}   (vrai : -0.7000)", calibrated.rho);
    println!("  v₀ = {:.4}   (vrai : 0.0400)", calibrated.v0);
    println!();
    println!("  Feller satisfait : {}", calibrated.feller_condition());
    println!("  RMSE final       : {:.6}", calibration::objective(&calibrated, &quotes, spot, rate));
}
