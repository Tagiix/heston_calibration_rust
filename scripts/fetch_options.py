#!/usr/bin/env python3
"""
Télécharge la chaîne d'options via yfinance et sauvegarde en CSV.

Usage:
    python scripts/fetch_options.py TICKER [OUTPUT_PATH]

Sortie CSV :
    strike,tau,implied_vol
    ...

Dernière ligne sur stdout :
    SPOT=<prix_spot>          ← lue par le programme Rust

Dépendances :
    pip install yfinance
"""

import sys
import os
import csv
import datetime

try:
    import yfinance as yf
except ImportError:
    print("Erreur : yfinance n'est pas installé.", file=sys.stderr)
    print("  pip install yfinance", file=sys.stderr)
    sys.exit(1)


def fetch_options(ticker: str, output_path: str) -> None:
    t = yf.Ticker(ticker)

    # Prix spot
    info = t.fast_info
    spot = info.get("last_price") or info.get("lastPrice")
    if spot is None or spot == 0:
        raise RuntimeError(
            f"Impossible de récupérer le prix spot pour « {ticker} ». "
            "Vérifiez le symbole."
        )

    # Dates d'expiration disponibles
    expirations = t.options
    if not expirations:
        raise RuntimeError(f"Aucune option disponible pour « {ticker} ».")

    now = datetime.datetime.now(datetime.timezone.utc).replace(tzinfo=None)

    quotes: list[tuple[float, float, float]] = []

    for exp_str in expirations[:6]:           # max 6 échéances
        exp_date = datetime.datetime.strptime(exp_str, "%Y-%m-%d")
        tau = (exp_date - now).days / 365.25

        if tau < 7 / 365 or tau > 2.0:        # < 1 semaine ou > 2 ans → ignoré
            continue

        try:
            chain = t.option_chain(exp_str)
        except Exception as exc:
            print(f"  Avertissement : échéance {exp_str} ignorée ({exc})", file=sys.stderr)
            continue

        calls = chain.calls

        for _, row in calls.iterrows():
            strike = float(row["strike"])
            iv = float(row["impliedVolatility"])
            volume = float(row.get("volume") or 0)

            moneyness = strike / spot
            if not (0.70 <= moneyness <= 1.30):
                continue
            if not (0.02 <= iv <= 3.0):         # IV entre 2 % et 300 %
                continue
            if volume < 5:
                continue

            quotes.append((strike, tau, iv))

    if not quotes:
        raise RuntimeError(
            "Aucune cotation utilisable trouvée "
            f"(ticker={ticker.upper()}, spot={spot:.2f}). "
            "Le sous-jacent a peut-être peu de liquidité sur les options."
        )

    # Sauvegarde CSV
    os.makedirs(os.path.dirname(output_path) or ".", exist_ok=True)
    with open(output_path, "w", newline="") as f:
        writer = csv.writer(f)
        writer.writerow(["strike", "tau", "implied_vol"])
        writer.writerows(quotes)

    print(
        f"  {len(quotes)} cotations sauvegardées pour {ticker.upper()}"
        f" → {output_path}",
        file=sys.stderr,
    )

    # Ligne parsée par Rust pour récupérer le spot
    print(f"SPOT={spot:.6f}")


if __name__ == "__main__":
    if len(sys.argv) < 2:
        print(__doc__)
        sys.exit(1)

    _ticker = sys.argv[1]
    _output = sys.argv[2] if len(sys.argv) > 2 else "data/market_surface.csv"

    try:
        fetch_options(_ticker, _output)
    except Exception as e:
        print(f"Erreur : {e}", file=sys.stderr)
        sys.exit(1)
