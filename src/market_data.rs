/// Données de marché : téléchargement live (yfinance via Python) et surface statique réaliste.
///
/// ## Usage CLI
///   `cargo run -- --ticker SPY`      → télécharge la chaîne d'options via yfinance
///   `cargo run -- --realistic`       → surface statique SPX stylisée (45 points)
///   `cargo run`                      → surface synthétique (comportement d'origine)
///
/// ## Pré-requis pour --ticker
///   pip install yfinance
use std::error::Error;

use crate::calibration::MarketQuote;

// ---------------------------------------------------------------------------
// Fetcher yfinance (via script Python)
// ---------------------------------------------------------------------------

const FETCH_SCRIPT: &str = "scripts/fetch_options.py";
const CSV_PATH: &str = "data/market_surface.csv";

/// Télécharge la chaîne d'options via le script Python `scripts/fetch_options.py`
/// (qui utilise `yfinance` en interne).
///
/// Le script :
///   1. Récupère spot + options pour plusieurs échéances
///   2. Filtre par moneyness (0.70–1.30), IV (2%–300%), volume (≥ 5)
///   3. Sauvegarde dans `data/market_surface.csv`
///   4. Imprime `SPOT=<valeur>` sur stdout pour que Rust récupère le prix spot
///
/// Retourne `(spot, Vec<MarketQuote>)`.
pub fn fetch_yahoo(ticker: &str) -> Result<(f64, Vec<MarketQuote>), Box<dyn Error>> {
    println!("  Appel de {} pour {}…", FETCH_SCRIPT, ticker.to_uppercase());

    // Utiliser le Python du venv si disponible, sinon retomber sur python3 système
    let python = if std::path::Path::new(".venv/bin/python3").exists() {
        ".venv/bin/python3"
    } else {
        "python3"
    };

    let output = std::process::Command::new(python)
        .args([FETCH_SCRIPT, ticker, CSV_PATH])
        .output()
        .map_err(|e| {
            format!(
                "Impossible de lancer {python} : {e}\n  \
                 Créez le venv : python3 -m venv .venv && .venv/bin/pip install yfinance"
            )
        })?;

    // Afficher stderr du script (messages de progression)
    let stderr = String::from_utf8_lossy(&output.stderr);
    for line in stderr.lines() {
        println!("  {line}");
    }

    if !output.status.success() {
        return Err(format!(
            "Le script Python a échoué (code {}).\n  \
             Installez yfinance si nécessaire : pip install yfinance",
            output.status
        )
        .into());
    }

    // Lire le spot depuis "SPOT=<valeur>" sur stdout
    let stdout = String::from_utf8_lossy(&output.stdout);
    let spot = stdout
        .lines()
        .find(|l| l.starts_with("SPOT="))
        .and_then(|l| l["SPOT=".len()..].trim().parse::<f64>().ok())
        .ok_or("Le script Python n'a pas renvoyé de ligne SPOT=<valeur>")?;

    // Charger le CSV produit par le script
    let quotes = crate::utils::load_surface(CSV_PATH)?;

    println!(
        "  {} cotations chargées pour {}  (spot = {:.2})",
        quotes.len(),
        ticker.to_uppercase(),
        spot
    );
    Ok((spot, quotes))
}

// ---------------------------------------------------------------------------
// Surface statique réaliste (style SPX)
// ---------------------------------------------------------------------------

/// Surface de volatilité implicite stylisée inspirée du marché SPX.
///
/// Caractéristiques :
///   - 5 maturités : 1M / 3M / 6M / 1Y / 2Y
///   - 9 strikes   : moneyness 0.80 à 1.20
///   - Skew négatif prononcé (puts OTM plus chers)
///   - Smile modéré (convexité)
///   - Structure par terme légèrement croissante en ATM / décroissante dans les ailes
///
/// Ces valeurs sont stylisées (non issues de données réelles) et servent
/// uniquement à tester la calibration dans des conditions plus réalistes
/// qu'une surface purement synthétique.
pub fn realistic_static_surface(spot: f64) -> Vec<MarketQuote> {
    // (moneyness, tau, implied_vol)
    let data: &[(f64, f64, f64)] = &[
        // ── 1 mois (τ ≈ 0.083) ──────────────────────────────────────────────
        (0.80, 0.083, 0.380),
        (0.85, 0.083, 0.320),
        (0.90, 0.083, 0.265),
        (0.95, 0.083, 0.225),
        (1.00, 0.083, 0.198),
        (1.05, 0.083, 0.188),
        (1.10, 0.083, 0.192),
        (1.15, 0.083, 0.205),
        (1.20, 0.083, 0.225),
        // ── 3 mois (τ = 0.25) ───────────────────────────────────────────────
        (0.80, 0.25, 0.320),
        (0.85, 0.25, 0.278),
        (0.90, 0.25, 0.242),
        (0.95, 0.25, 0.215),
        (1.00, 0.25, 0.196),
        (1.05, 0.25, 0.186),
        (1.10, 0.25, 0.186),
        (1.15, 0.25, 0.194),
        (1.20, 0.25, 0.210),
        // ── 6 mois (τ = 0.50) ───────────────────────────────────────────────
        (0.80, 0.50, 0.290),
        (0.85, 0.50, 0.258),
        (0.90, 0.50, 0.232),
        (0.95, 0.50, 0.212),
        (1.00, 0.50, 0.198),
        (1.05, 0.50, 0.190),
        (1.10, 0.50, 0.190),
        (1.15, 0.50, 0.196),
        (1.20, 0.50, 0.208),
        // ── 1 an (τ = 1.00) ─────────────────────────────────────────────────
        (0.80, 1.00, 0.268),
        (0.85, 1.00, 0.245),
        (0.90, 1.00, 0.226),
        (0.95, 1.00, 0.212),
        (1.00, 1.00, 0.203),
        (1.05, 1.00, 0.198),
        (1.10, 1.00, 0.197),
        (1.15, 1.00, 0.201),
        (1.20, 1.00, 0.210),
        // ── 2 ans (τ = 2.00) ────────────────────────────────────────────────
        (0.80, 2.00, 0.255),
        (0.85, 2.00, 0.238),
        (0.90, 2.00, 0.224),
        (0.95, 2.00, 0.214),
        (1.00, 2.00, 0.208),
        (1.05, 2.00, 0.205),
        (1.10, 2.00, 0.204),
        (1.15, 2.00, 0.207),
        (1.20, 2.00, 0.213),
    ];

    data.iter()
        .map(|&(m, tau, iv)| MarketQuote { strike: spot * m, tau, implied_vol: iv })
        .collect()
}
