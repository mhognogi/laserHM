//! Generează G-code-ul pentru o lucrare.

/// O linie dreaptă pe axa X, pornind din poziția curentă a capului.
pub struct LineJob {
    pub length_mm: f64,
    /// Procent din puterea maximă a laserului (setarea GRBL `$30`).
    pub power_percent: f64,
    pub feed_mm_per_min: f64,
}

/// Valori de exemplu, doar ca să avem de unde porni.
pub const EXAMPLE_LINE: LineJob = LineJob {
    length_mm: 20.0,
    power_percent: 30.0,
    feed_mm_per_min: 2000.0,
};

impl LineJob {
    /// `max_power` este valoarea `$30` citită din laser.
    pub fn gcode(&self, max_power: f64) -> Vec<String> {
        let power = (max_power * self.power_percent / 100.0).round();
        vec![
            // Milimetri, coordonate relative (față de poziția curentă).
            "G21 G91".to_string(),
            // M4 = putere dinamică, ca în LightBurn: GRBL scade puterea când
            // mașina încetinește, ca arderea să fie uniformă.
            "M4 S0".to_string(),
            format!("G1 X{} S{power} F{}", self.length_mm, self.feed_mm_per_min),
            // Laser oprit, înapoi la coordonate absolute.
            "M5 S0".to_string(),
            "G90".to_string(),
            // Pauză de 0 s: GRBL răspunde "ok" abia după ce mișcarea s-a terminat,
            // așa știm când lucrarea e gata cu adevărat.
            "G4 P0".to_string(),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn example_line_at_max_power_1000() {
        assert_eq!(
            EXAMPLE_LINE.gcode(1000.0),
            ["G21 G91", "M4 S0", "G1 X20 S300 F2000", "M5 S0", "G90", "G4 P0"]
        );
    }

    #[test]
    fn power_follows_max_power_setting() {
        assert_eq!(EXAMPLE_LINE.gcode(255.0)[2], "G1 X20 S77 F2000");
    }
}
