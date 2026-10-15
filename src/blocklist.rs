//! Ad and tracker blocking through WebKit's content filters.
//!
//! Third-party requests to the hosts below are blocked, plus a few obvious ad
//! containers are hidden. Put extra hosts (one per line) in `blocklist.txt`.

const HOSTS: &[&str] = &[
    // ad networks & exchanges
    "doubleclick.net", "googlesyndication.com", "googleadservices.com", "adservice.google.com",
    "adnxs.com", "adsrvr.org", "advertising.com", "adform.net", "criteo.com", "criteo.net",
    "taboola.com", "outbrain.com", "revcontent.com", "mgid.com", "pubmatic.com", "rubiconproject.com",
    "openx.net", "casalemedia.com", "indexww.com", "sharethrough.com", "smartadserver.com",
    "yieldmo.com", "media.net", "amazon-adsystem.com", "adsafeprotected.com", "moatads.com",
    "2mdn.net", "serving-sys.com", "bidswitch.net", "contextweb.com", "lijit.com", "sovrn.com",
    "teads.tv", "33across.com", "gumgum.com", "triplelift.com", "tapad.com", "zemanta.com",
    "adcolony.com", "applovin.com", "inmobi.com", "mopub.com", "popads.net", "propellerads.com",
    "exoclick.com", "juicyads.com", "trafficjunky.net", "adskeeper.com", "ad.gt", "adition.com",
    // analytics & tracking
    "google-analytics.com", "googletagmanager.com", "googletagservices.com", "scorecardresearch.com",
    "quantserve.com", "quantcount.com", "chartbeat.com", "chartbeat.net", "hotjar.com", "hotjar.io",
    "mouseflow.com", "crazyegg.com", "fullstory.com", "luckyorange.com", "inspectlet.com",
    "mixpanel.com", "segment.io", "segment.com", "amplitude.com", "heap.io", "heapanalytics.com",
    "newrelic.com", "nr-data.net", "bugsnag.com", "branch.io", "appsflyer.com", "adjust.com",
    "kochava.com", "bluekai.com", "demdex.net", "everesttech.net", "omtrdc.net", "2o7.net",
    "krxd.net", "agkn.com", "rlcdn.com", "liveramp.com", "id5-sync.com", "crwdcntrl.net",
    "exelator.com", "eyeota.net", "mathtag.com", "turn.com", "mookie1.com", "rfihub.com",
    "pippio.com", "bidr.io", "dotomi.com", "yimg.com/aaq", "ads-twitter.com", "analytics.twitter.com",
    "px.ads.linkedin.com", "snap.licdn.com", "analytics.tiktok.com", "ads.tiktok.com",
    "connect.facebook.net", "pixel.facebook.com", "an.facebook.com", "analytics.yahoo.com",
    "ads.yahoo.com", "log.pinterest.com", "ct.pinterest.com", "tr.snapchat.com", "sc-static.net",
    "clarity.ms", "bat.bing.com", "optimizely.com", "vwo.com", "abtasty.com", "tiqcdn.com",
    "browser-update.org", "outbrainimg.com", "disqusads.com", "ml314.com", "tags.tiqcdn.com",
    "cdn.taboola.com", "trc.taboola.com", "sb.scorecardresearch.com", "pagead2.googlesyndication.com",
];

/// Obvious ad containers; kept short on purpose so pages are not broken.
const HIDE: &str = "ins.adsbygoogle, .adsbygoogle, [id^='google_ads_iframe'], [id^='div-gpt-ad'], \
[id^='google_ads_'], iframe[src*='doubleclick.net'], iframe[src*='googlesyndication.com'], \
.ad-slot, .ad-banner, .ad-container, .advert, .advertisement, .sponsored-ad, #ad-banner, \
[data-ad-slot], [data-google-query-id]";

fn json_str(s: &str) -> String {
    let mut o = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            c if (c as u32) < 0x20 => o.push(' '),
            c => o.push(c),
        }
    }
    o.push('"');
    o
}

fn host_rule(host: &str) -> String {
    let escaped = host.replace('.', "\\.");
    let filter = format!("^[^:]+://+([^:/]+\\.)?{escaped}[:/]");
    format!(
        "{{\"trigger\":{{\"url-filter\":{},\"load-type\":[\"third-party\"]}},\"action\":{{\"type\":\"block\"}}}}",
        json_str(&filter)
    )
}

/// The rule list in WebKit's content-blocker JSON format.
pub fn rules_json(extra: &str) -> String {
    let mut rules: Vec<String> = HOSTS
        .iter()
        .filter(|h| !h.contains('/'))
        .map(|h| host_rule(h))
        .collect();
    for line in extra.lines() {
        let h = line.split('#').next().unwrap_or("").trim().trim_start_matches("*.");
        if !h.is_empty() && h.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-') {
            rules.push(host_rule(h));
        }
    }
    for path in ["/pagead/ads", "/pagead/js"] {
        rules.push(format!("{{\"trigger\":{{\"url-filter\":{}}},\"action\":{{\"type\":\"block\"}}}}", json_str(path)));
    }
    rules.push(format!(
        "{{\"trigger\":{{\"url-filter\":\".*\"}},\"action\":{{\"type\":\"css-display-none\",\"selector\":{}}}}}",
        json_str(HIDE)
    ));
    format!("[{}]", rules.join(","))
}
