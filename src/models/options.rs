use statrs::distribution::{Normal, Continuous, ContinuousCDF};

/// Option type
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum OptionType {
    Call,
    Put,
}

/// Greeks for options
#[derive(Debug, Clone, Copy)]
pub struct Greeks {
    pub delta: f64,
    pub gamma: f64,
    pub vega: f64,
    pub theta: f64,
    pub rho: f64,
}

/// Black-Scholes Option Pricer
/// Used for pricing and hedging Deribit options
pub struct BlackScholes {
    normal: Normal,
}

impl BlackScholes {
    pub fn new() -> Self {
        Self {
            normal: Normal::new(0.0, 1.0).unwrap(),
        }
    }

    /// Calculate option price using Black-Scholes formula
    ///
    /// # Arguments
    /// * `spot` - Current spot price
    /// * `strike` - Strike price
    /// * `time_to_expiry` - Time to expiration (in years)
    /// * `volatility` - Implied volatility (annualized)
    /// * `risk_free_rate` - Risk-free interest rate (annualized)
    /// * `option_type` - Call or Put
    pub fn price(
        &self,
        spot: f64,
        strike: f64,
        time_to_expiry: f64,
        volatility: f64,
        risk_free_rate: f64,
        option_type: OptionType,
    ) -> f64 {
        if time_to_expiry <= 0.0 {
            return self.intrinsic_value(spot, strike, option_type);
        }

        let (d1, d2) = self.calculate_d1_d2(
            spot,
            strike,
            time_to_expiry,
            volatility,
            risk_free_rate,
        );

        match option_type {
            OptionType::Call => {
                spot * self.normal.cdf(d1)
                    - strike * (-risk_free_rate * time_to_expiry).exp() * self.normal.cdf(d2)
            }
            OptionType::Put => {
                strike * (-risk_free_rate * time_to_expiry).exp() * self.normal.cdf(-d2)
                    - spot * self.normal.cdf(-d1)
            }
        }
    }

    /// Calculate all Greeks
    pub fn greeks(
        &self,
        spot: f64,
        strike: f64,
        time_to_expiry: f64,
        volatility: f64,
        risk_free_rate: f64,
        option_type: OptionType,
    ) -> Greeks {
        if time_to_expiry <= 0.0 {
            return Greeks {
                delta: self.delta_at_expiry(spot, strike, option_type),
                gamma: 0.0,
                vega: 0.0,
                theta: 0.0,
                rho: 0.0,
            };
        }

        let delta = self.delta(spot, strike, time_to_expiry, volatility, risk_free_rate, option_type);
        let gamma = self.gamma(spot, strike, time_to_expiry, volatility, risk_free_rate);
        let vega = self.vega(spot, strike, time_to_expiry, volatility, risk_free_rate);
        let theta = self.theta(spot, strike, time_to_expiry, volatility, risk_free_rate, option_type);
        let rho = self.rho(spot, strike, time_to_expiry, volatility, risk_free_rate, option_type);

        Greeks {
            delta,
            gamma,
            vega,
            theta,
            rho,
        }
    }

    /// Calculate Delta (sensitivity to spot price)
    pub fn delta(
        &self,
        spot: f64,
        strike: f64,
        time_to_expiry: f64,
        volatility: f64,
        risk_free_rate: f64,
        option_type: OptionType,
    ) -> f64 {
        if time_to_expiry <= 0.0 {
            return self.delta_at_expiry(spot, strike, option_type);
        }

        let (d1, _) = self.calculate_d1_d2(
            spot,
            strike,
            time_to_expiry,
            volatility,
            risk_free_rate,
        );

        match option_type {
            OptionType::Call => self.normal.cdf(d1),
            OptionType::Put => self.normal.cdf(d1) - 1.0,
        }
    }

    /// Calculate Gamma (sensitivity of delta to spot price)
    pub fn gamma(
        &self,
        spot: f64,
        strike: f64,
        time_to_expiry: f64,
        volatility: f64,
        risk_free_rate: f64,
    ) -> f64 {
        if time_to_expiry <= 0.0 {
            return 0.0;
        }

        let (d1, _) = self.calculate_d1_d2(
            spot,
            strike,
            time_to_expiry,
            volatility,
            risk_free_rate,
        );

        self.normal.pdf(d1) / (spot * volatility * time_to_expiry.sqrt())
    }

    /// Calculate Vega (sensitivity to volatility)
    pub fn vega(
        &self,
        spot: f64,
        strike: f64,
        time_to_expiry: f64,
        volatility: f64,
        risk_free_rate: f64,
    ) -> f64 {
        if time_to_expiry <= 0.0 {
            return 0.0;
        }

        let (d1, _) = self.calculate_d1_d2(
            spot,
            strike,
            time_to_expiry,
            volatility,
            risk_free_rate,
        );

        spot * self.normal.pdf(d1) * time_to_expiry.sqrt() / 100.0 // Divide by 100 for 1% vol change
    }

    /// Calculate Theta (time decay)
    pub fn theta(
        &self,
        spot: f64,
        strike: f64,
        time_to_expiry: f64,
        volatility: f64,
        risk_free_rate: f64,
        option_type: OptionType,
    ) -> f64 {
        if time_to_expiry <= 0.0 {
            return 0.0;
        }

        let (d1, d2) = self.calculate_d1_d2(
            spot,
            strike,
            time_to_expiry,
            volatility,
            risk_free_rate,
        );

        let term1 = -(spot * self.normal.pdf(d1) * volatility) / (2.0 * time_to_expiry.sqrt());

        match option_type {
            OptionType::Call => {
                let term2 = risk_free_rate * strike * (-risk_free_rate * time_to_expiry).exp() * self.normal.cdf(d2);
                (term1 - term2) / 365.0 // Convert to daily theta
            }
            OptionType::Put => {
                let term2 = risk_free_rate * strike * (-risk_free_rate * time_to_expiry).exp() * self.normal.cdf(-d2);
                (term1 + term2) / 365.0 // Convert to daily theta
            }
        }
    }

    /// Calculate Rho (sensitivity to interest rate)
    pub fn rho(
        &self,
        spot: f64,
        strike: f64,
        time_to_expiry: f64,
        volatility: f64,
        risk_free_rate: f64,
        option_type: OptionType,
    ) -> f64 {
        if time_to_expiry <= 0.0 {
            return 0.0;
        }

        let (_, d2) = self.calculate_d1_d2(
            spot,
            strike,
            time_to_expiry,
            volatility,
            risk_free_rate,
        );

        match option_type {
            OptionType::Call => {
                strike * time_to_expiry * (-risk_free_rate * time_to_expiry).exp() * self.normal.cdf(d2) / 100.0
            }
            OptionType::Put => {
                -strike * time_to_expiry * (-risk_free_rate * time_to_expiry).exp() * self.normal.cdf(-d2) / 100.0
            }
        }
    }

    /// Calculate d1 and d2 for Black-Scholes formula
    fn calculate_d1_d2(
        &self,
        spot: f64,
        strike: f64,
        time_to_expiry: f64,
        volatility: f64,
        risk_free_rate: f64,
    ) -> (f64, f64) {
        let d1 = ((spot / strike).ln() + (risk_free_rate + volatility.powi(2) / 2.0) * time_to_expiry)
            / (volatility * time_to_expiry.sqrt());

        let d2 = d1 - volatility * time_to_expiry.sqrt();

        (d1, d2)
    }

    /// Intrinsic value at expiry
    fn intrinsic_value(&self, spot: f64, strike: f64, option_type: OptionType) -> f64 {
        match option_type {
            OptionType::Call => (spot - strike).max(0.0),
            OptionType::Put => (strike - spot).max(0.0),
        }
    }

    /// Delta at expiry
    fn delta_at_expiry(&self, spot: f64, strike: f64, option_type: OptionType) -> f64 {
        match option_type {
            OptionType::Call => {
                if spot > strike {
                    1.0
                } else {
                    0.0
                }
            }
            OptionType::Put => {
                if spot < strike {
                    -1.0
                } else {
                    0.0
                }
            }
        }
    }

    /// Calculate implied volatility using Newton-Raphson method
    pub fn implied_volatility(
        &self,
        spot: f64,
        strike: f64,
        time_to_expiry: f64,
        risk_free_rate: f64,
        option_price: f64,
        option_type: OptionType,
        max_iterations: usize,
    ) -> Option<f64> {
        let mut vol = 0.5; // Initial guess
        let tolerance = 0.0001;

        for _ in 0..max_iterations {
            let price = self.price(spot, strike, time_to_expiry, vol, risk_free_rate, option_type);
            let vega = self.vega(spot, strike, time_to_expiry, vol, risk_free_rate);

            if vega.abs() < 1e-10 {
                return None;
            }

            let diff = price - option_price;

            if diff.abs() < tolerance {
                return Some(vol);
            }

            vol -= diff / (vega * 100.0); // Multiply vega back by 100

            if vol <= 0.0 || vol > 5.0 {
                return None;
            }
        }

        None
    }
}

impl Default for BlackScholes {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_call_price() {
        let bs = BlackScholes::new();

        let price = bs.price(
            100.0,  // spot
            100.0,  // strike (ATM)
            1.0,    // 1 year
            0.2,    // 20% vol
            0.05,   // 5% risk-free rate
            OptionType::Call,
        );

        // ATM call with 1 year should have significant time value
        assert!(price > 10.0 && price < 15.0);
    }

    #[test]
    fn test_put_call_parity() {
        let bs = BlackScholes::new();

        let spot = 100.0;
        let strike = 100.0;
        let time = 1.0;
        let vol = 0.2;
        let rate = 0.05;

        let call = bs.price(spot, strike, time, vol, rate, OptionType::Call);
        let put = bs.price(spot, strike, time, vol, rate, OptionType::Put);

        // Put-Call Parity: C - P = S - K*e^(-rT)
        let lhs = call - put;
        let rhs = spot - strike * (-rate * time).exp();

        assert!((lhs - rhs).abs() < 0.01);
    }

    #[test]
    fn test_delta_bounds() {
        let bs = BlackScholes::new();

        // Call delta should be between 0 and 1
        let call_delta = bs.delta(100.0, 100.0, 1.0, 0.2, 0.05, OptionType::Call);
        assert!(call_delta > 0.0 && call_delta < 1.0);

        // Put delta should be between -1 and 0
        let put_delta = bs.delta(100.0, 100.0, 1.0, 0.2, 0.05, OptionType::Put);
        assert!(put_delta < 0.0 && put_delta > -1.0);
    }
}
