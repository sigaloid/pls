use std::process::Stdio;

use pickledb::PickleDb;
use spinach::Spinach;

use crate::get_time;

pub(crate) fn get_weather(db: &mut PickleDb, force_refresh: bool) -> Option<String> {
    // represent current unix timestamp
    let timestamp_current = get_time().unix_timestamp();
    // closure that fetches the weather and caches it.
    let fetch_and_cache_weather = |db: &mut PickleDb| -> Option<String> {
        // if specific location is not set, the default for `String` will be used (an empty string).
        // thus the request will be to "https://wttr.in/?format=%l:+%C+%c+%t" which is the URL structure
        // for letting the server geolocate based on IP address.
        let s = Spinach::new("Getting weather location from database...");

        let specific_location = db
            .get::<String>("weather-specific-location")
            .unwrap_or_default();

        s.text(format!(
            "Getting weather for {} from weather service...",
            specific_location
        ));

        let weather_info = ureq::get(&format!(
            "https://wttr.in/{}?format=%l:+%C+%c+%t",
            specific_location
        ))
        .call()
        .ok()?
        .into_string()
        .ok()?;

        s.text("Caching weather...");

        db.set("weather-cached", &weather_info)
            .expect("Failed to set cached weather");

        s.text("Caching weather timestamp...");

        db.set("weather-timestamp", &timestamp_current)
            .expect("Failed to set cached weather");

        s.succeed("Weather retrieved");
        Some(weather_info)
    };
    // if weather-timestamp is set (ie previous cache success)
    if let Some(timestamp) = db.get::<i64>("weather-timestamp") {
        // if manually forcing a refresh
        if force_refresh {
            // force refresh and block thread when forced
            fetch_and_cache_weather(db)
        } else if timestamp_current - timestamp > 3600 || !db.exists("weather-cached") {
            // if refresh isn't forced, but it is outdated or a cache doesn't exist, spawn new
            // process to update in the background, so that the terminal isn't blocked by a weather
            // update, but when the user next uses `pls`, they will receive up-to-date weather.
            drop(
                std::process::Command::new("pls")
                    .arg("-r")
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .spawn(),
            );
            // then report a cached version (and if there is none, just use an empty string. The next time it will contain actual weather)
            Some(
                db.get::<String>("weather-cached")
                    .map(|s| {
                        format!(
                            "{} ({} min outdated, will be updated on next launch)",
                            s,
                            (timestamp_current - timestamp) / 60
                        )
                    })
                    .unwrap_or_default(),
            )
        } else {
            // if the timestamp is not outdated simply load cached weather
            db.get::<String>("weather-cached")
        }
    } else {
        // if no previous cached version, simply load weather
        fetch_and_cache_weather(db)
    }
}
