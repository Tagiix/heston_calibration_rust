/// Utilitaires : chargement de données, affichage, génération de surfaces synthétiques.
use std::error::Error;
use crate::calibration::MarketQuote;

// ---------------------------------------------------------------------------
// Chargement depuis CSV
// ---------------------------------------------------------------------------

/// Charge une surface de volatilité implicite depuis un fichier CSV.
///
/// Format attendu (une ligne d'en-tête + lignes de données) :
///   strike,tau,implied_vol
///   80.0,0.25,0.285
///   ...
///
/// Retourne une `Vec<MarketQuote>` ou une erreur de parsing.
pub fn load_surface(path: &str) -> Result<Vec<MarketQuote>, Box<dyn Error>> {
    let mut reader = csv::Reader::from_path(path)?;
    let mut quotes = Vec::new();

    for result in reader.records() {
        let record = result?;
        let strike: f64 = record[0].trim().parse()?;
        let tau: f64 = record[1].trim().parse()?;
        let implied_vol: f64 = record[2].trim().parse()?;
        quotes.push(MarketQuote { strike, tau, implied_vol });
    }

    Ok(quotes)
}

// ---------------------------------------------------------------------------
// Génération d'une surface synthétique
// ---------------------------------------------------------------------------

/// Génère une surface de volatilité implicite synthétique.
///
/// La surface est construite à partir d'une forme fonctionnelle simple
/// qui reproduit les caractéristiques typiques du marché :
///   - Un sourire de volatilité (smile)
///   - Une asymétrie (skew) : les puts OTM ont une IV plus élevée
///   - Une structure par terme croissante avec la maturité
///
/// Cette surface peut servir de données fictives pour tester la calibration.
pub fn generate_synthetic_surface(spot: f64) -> Vec<MarketQuote> {
    let strikes = [0.80, 0.85, 0.90, 0.95, 1.00, 1.05, 1.10];
    let taus    = [0.25_f64, 0.5, 1.0];

    let mut quotes = Vec::new();

    for &tau in &taus {
        for &moneyness in &strikes {
            let strike = spot * moneyness;

            // Volatilité implicite synthétique avec smile et skew
            // (approximation analytique grossière, non issue d'un modèle exact)
            let atm_vol  = 0.20 + 0.01 * tau.sqrt();           // légère structure par terme
            let skew     = -0.10 * (moneyness - 1.0);           // skew négatif
            let smile    = 0.05 * (moneyness - 1.0).powi(2);   // courbure (smile)
            let iv = (atm_vol + skew + smile).max(0.05);        // IV ≥ 5% par sécurité

            quotes.push(MarketQuote { strike, tau, implied_vol: iv });
        }
    }

    quotes
}

// ---------------------------------------------------------------------------
// Sauvegarde en CSV
// ---------------------------------------------------------------------------

/// Sauvegarde une surface de volatilité implicite dans un fichier CSV.
///
/// Format : en-tête `strike,tau,implied_vol` suivi des données.
/// Crée le dossier parent si nécessaire.
pub fn save_surface(path: &str, quotes: &[MarketQuote]) -> Result<(), Box<dyn Error>> {
    if let Some(parent) = std::path::Path::new(path).parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut wtr = csv::Writer::from_path(path)?;
    wtr.write_record(["strike", "tau", "implied_vol"])?;
    for q in quotes {
        wtr.write_record(&[
            q.strike.to_string(),
            q.tau.to_string(),
            q.implied_vol.to_string(),
        ])?;
    }
    wtr.flush()?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Affichage
// ---------------------------------------------------------------------------

/// Affiche une surface de volatilité implicite sous forme de tableau.
pub fn print_vol_surface(quotes: &[MarketQuote]) {
    println!("{:<10} {:<8} {:<12}", "Strike", "τ (an)", "IV (%)");
    println!("{}", "-".repeat(32));
    for q in quotes {
        println!("{:<10.1} {:<8.2} {:<12.2}", q.strike, q.tau, q.implied_vol * 100.0);
    }
}
