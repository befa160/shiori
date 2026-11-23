//! Relais de l'application Android (lot 1.1c, Q62) : la WebView charge `https://shiori.localhost/`,
//! et chaque requête (interface comprise) est transmise telle quelle au serveur Shiori réglé. Pour
//! la WebView tout est sur la même origine : le cookie de session reste dans son stockage, quelle
//! que soit l'adresse du serveur, et le serveur n'a rien à changer.

use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;

use http::{HeaderValue, Method, Request, Response, StatusCode, Uri, header};

/// Page qui règle l'adresse du serveur (pas de serveur réglé, ou serveur injoignable).
const SETUP: &str = "/__shiori/setup";
/// Plus gros corps de réponse accepté (une sauvegarde téléchargée, par exemple).
const MAX_BODY: u64 = 4 << 30;
/// En-têtes propres à une connexion, jamais retransmis (RFC 9110, § 7.6.1), et ceux que
/// l'agent HTTP recalcule lui-même.
const HOP_BY_HOP: [header::HeaderName; 6] = [
    header::CONNECTION,
    header::HOST,
    header::CONTENT_LENGTH,
    header::TRANSFER_ENCODING,
    header::TE,
    header::UPGRADE,
];
/// Requêtes conditionnelles : elles appellent un 304, qu'une réponse interceptée ne peut pas
/// être sous Android (voir `handle`). Le serveur renvoie donc toujours la réponse entière.
const CONDITIONAL: [header::HeaderName; 2] = [header::IF_NONE_MATCH, header::IF_MODIFIED_SINCE];

pub struct Relay {
    agent: ureq::Agent,
    /// Fichier qui garde l'adresse du serveur, dans le dossier privé de l'application.
    saved: PathBuf,
    server: Mutex<Option<String>>,
    /// Jeton tiré à chaque lancement, caché dans le formulaire de réglage : seul ce formulaire
    /// change le serveur. Une page étrangère chargée dans la WebView ne le lit pas, et ne peut
    /// donc pas détourner l'application (et la session) vers son propre serveur. (La WebView
    /// d'Android n'envoie pas `Sec-Fetch-Site` aux requêtes interceptées : pas de garde par là.)
    token: String,
}

impl Relay {
    /// Relais dont l'adresse est gardée dans `dir` (créé au besoin).
    pub fn new(dir: PathBuf) -> Self {
        let saved = dir.join("server.txt");
        let server = std::fs::read_to_string(&saved)
            .ok()
            .and_then(|s| server_address(s.trim()));
        let agent = ureq::Agent::config_builder()
            // Erreurs HTTP et redirections reviennent telles quelles à l'interface.
            .http_status_as_error(false)
            .max_redirects(0)
            .max_redirects_will_error(false)
            .timeout_connect(Some(Duration::from_secs(5)))
            .build()
            .into();
        Self {
            agent,
            saved,
            server: Mutex::new(server),
            token: random_token(),
        }
    }

    /// Répond à une requête de la WebView : page de réglage, ou réponse du serveur.
    pub fn handle(&self, request: Request<Vec<u8>>) -> Response<Vec<u8>> {
        let french = request
            .headers()
            .get(header::ACCEPT_LANGUAGE)
            .is_some_and(|v| v.as_bytes().starts_with(b"fr"));
        let texts = if french { &FR } else { &EN };
        let server = self.server.lock().expect("adresse du serveur").clone();
        if request.uri().path() == SETUP {
            return self.setup(request.uri(), server.as_deref(), texts);
        }
        let Some(server) = server else {
            return self.page(StatusCode::OK, texts, None, "");
        };
        // shortcut: pas de service worker dans l'application (il ne passerait pas par le relais),
        // donc pas de hors-ligne ; le relais le remplacera au lot 1.1e.
        if request.uri().path() == "/sw.js" {
            return text(StatusCode::NOT_FOUND, "");
        }
        match self.forward(&server, request) {
            // La WebView d'Android refuse une réponse interceptée en 3xx (elle charge alors
            // l'adresse sur le réseau, où rien n'écoute) : une redirection ne la traverse jamais.
            // Le serveur Shiori n'en envoie pas ; un proxy, si (http vers https) : on propose
            // l'adresse vers laquelle il renvoie.
            Ok(response) if response.status().is_redirection() => {
                let target = response
                    .headers()
                    .get(header::LOCATION)
                    .and_then(|v| v.to_str().ok())
                    .and_then(|l| l.parse::<Uri>().ok())
                    .and_then(|uri| Some(format!("{}://{}", uri.scheme_str()?, uri.authority()?)));
                self.page(
                    StatusCode::BAD_GATEWAY,
                    texts,
                    Some(texts.redirected),
                    target.as_deref().unwrap_or(&server),
                )
            }
            Ok(response) => response,
            Err(_) => self.page(
                StatusCode::BAD_GATEWAY,
                texts,
                Some(texts.unreachable),
                &server,
            ),
        }
    }

    /// `GET /__shiori/setup?server=…&token=…` enregistre l'adresse et ouvre l'interface ; sans
    /// adresse valable ou sans le jeton du formulaire, la page de réglage (adresse actuelle).
    fn setup(&self, uri: &Uri, current: Option<&str>, texts: &Texts) -> Response<Vec<u8>> {
        let param = |name: &str| {
            uri.query()?
                .split('&')
                .find_map(|p| p.strip_prefix(name)?.strip_prefix('='))
                .map(form_decode)
        };
        let asked = param("server").filter(|_| param("token").as_deref() == Some(&self.token));
        let Some(asked) = asked else {
            return self.page(StatusCode::OK, texts, None, current.unwrap_or(""));
        };
        let Some(server) = server_address(asked.trim()) else {
            return self.page(StatusCode::BAD_REQUEST, texts, Some(texts.invalid), &asked);
        };
        if std::fs::write(&self.saved, &server).is_err() {
            return self.page(
                StatusCode::INTERNAL_SERVER_ERROR,
                texts,
                Some(texts.not_saved),
                &server,
            );
        }
        *self.server.lock().expect("adresse du serveur") = Some(server);
        // Pas de 303 (refusé par la WebView d'Android, voir `handle`) : une page qui ouvre l'accueil.
        let mut response = text(
            StatusCode::OK,
            r#"<!doctype html><meta http-equiv="refresh" content="0;url=/"><script>location.replace('/')</script>"#,
        );
        response.headers_mut().insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("text/html; charset=utf-8"),
        );
        response
    }

    fn forward(
        &self,
        server: &str,
        request: Request<Vec<u8>>,
    ) -> Result<Response<Vec<u8>>, ureq::Error> {
        let (parts, body) = request.into_parts();
        let path = parts.uri.path_and_query().map_or("/", |p| p.as_str());
        let mut outgoing = Request::builder()
            .method(parts.method.clone())
            .uri(format!("{server}{path}"));
        for (name, value) in &parts.headers {
            if !HOP_BY_HOP.contains(name) && !CONDITIONAL.contains(name) {
                outgoing = outgoing.header(name, value);
            }
        }
        // Un GET n'a pas de corps : ne pas en annoncer un vide au serveur.
        let response = match parts.method == Method::GET || parts.method == Method::HEAD {
            true => self.agent.run(outgoing.body(())?)?,
            false => self.agent.run(outgoing.body(body)?)?,
        };
        let (parts, body) = response.into_parts();
        let body = body.into_with_config().limit(MAX_BODY).read_to_vec()?;
        let mut response = Response::new(body);
        *response.status_mut() = parts.status;
        for (name, value) in &parts.headers {
            if !HOP_BY_HOP.contains(name) {
                response.headers_mut().append(name, value.clone());
            }
        }
        Ok(response)
    }
}

/// Adresse de serveur acceptée : `http://` ou `https://`, un hôte, rien après (pas de chemin,
/// de paramètres ni de fragment). Rendue sans `/` final.
fn server_address(input: &str) -> Option<String> {
    let uri: Uri = input.parse().ok()?;
    let scheme = uri.scheme_str()?;
    let path_ok = matches!(uri.path(), "" | "/") && uri.query().is_none() && !input.contains('#');
    let host_ok = uri.host().is_some_and(|h| !h.is_empty());
    (matches!(scheme, "http" | "https") && host_ok && path_ok)
        .then(|| input.trim_end_matches('/').to_owned())
}

/// Décode une valeur de formulaire (`application/x-www-form-urlencoded`) : `+` et `%XX`.
fn form_decode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let hex = |b: u8| (b as char).to_digit(16);
        match bytes[i] {
            b'+' => out.push(b' '),
            b'%' if i + 2 < bytes.len() => match (hex(bytes[i + 1]), hex(bytes[i + 2])) {
                (Some(h), Some(l)) => {
                    out.push((h * 16 + l) as u8);
                    i += 2;
                }
                _ => out.push(b'%'),
            },
            b => out.push(b),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

struct Texts {
    lang: &'static str,
    title: &'static str,
    help: &'static str,
    label: &'static str,
    save: &'static str,
    invalid: &'static str,
    unreachable: &'static str,
    redirected: &'static str,
    not_saved: &'static str,
}

const FR: Texts = Texts {
    lang: "fr",
    title: "Adresse du serveur Shiori",
    help: "L'adresse que vous ouvrez dans le navigateur, par exemple http://192.168.1.20:7878 ou https://shiori.exemple.fr.",
    label: "Adresse",
    save: "Ouvrir",
    invalid: "Adresse non valable : http:// ou https://, puis le nom ou l'adresse IP du serveur, sans chemin.",
    unreachable: "Le serveur ne répond pas à cette adresse. Vérifiez le réseau ou l'adresse.",
    redirected: "Cette adresse renvoie vers une autre (souvent https://) : la voici, ouvrez-la si c'est bien votre serveur.",
    not_saved: "L'adresse n'a pas pu être enregistrée sur l'appareil.",
};

const EN: Texts = Texts {
    lang: "en",
    title: "Shiori server address",
    help: "The address you open in the browser, for example http://192.168.1.20:7878 or https://shiori.example.com.",
    label: "Address",
    save: "Open",
    invalid: "Invalid address: http:// or https://, then the server name or IP address, without a path.",
    unreachable: "The server does not answer at this address. Check the network or the address.",
    redirected: "This address sends to another one (often https://): here it is, open it if it is your server.",
    not_saved: "The address could not be saved on the device.",
};

impl Relay {
    /// Page de réglage de l'adresse, avec un message d'erreur éventuel.
    fn page(
        &self,
        status: StatusCode,
        t: &Texts,
        error: Option<&str>,
        value: &str,
    ) -> Response<Vec<u8>> {
        let error = error.map_or(String::new(), |e| {
            format!(r#"<p role="alert" class="error">{}</p>"#, escape(e))
        });
        let html = format!(
            r#"<!doctype html>
    <html lang="{lang}"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
    <title>Shiori</title><style>
    :root{{color-scheme:dark;font-family:system-ui,sans-serif;background:#121212;color:#f4f4f5}}
    body{{margin:0;padding:max(1.5rem,env(safe-area-inset-top)) 1rem 1.5rem;max-width:32rem;margin-inline:auto}}
    h1{{font-size:1.5rem}}p{{color:#a1a1aa;line-height:1.5}}.error{{color:#f87171}}
    label{{display:block;font-weight:600;margin:1.5rem 0 .5rem}}
    input{{box-sizing:border-box;width:100%;min-height:44px;padding:.5rem .75rem;border-radius:.375rem;border:1px solid #3f3f46;background:#1e1e1e;color:inherit;font-size:1rem}}
    button{{margin-top:1rem;min-height:44px;padding:0 1.25rem;border:0;border-radius:.375rem;background:#e5a00d;color:#121212;font-weight:600;font-size:1rem}}
    </style></head><body><h1>{title}</h1><p>{help}</p>{error}
    <form method="get" action="{SETUP}"><label for="server">{label}</label>
    <input type="hidden" name="token" value="{token}">
<input id="server" name="server" type="url" inputmode="url" autocomplete="url" required value="{value}">
    <button type="submit">{save}</button></form></body></html>"#,
            lang = t.lang,
            title = t.title,
            help = escape(t.help),
            label = t.label,
            save = t.save,
            value = escape(value),
            token = self.token,
        );
        let mut response = text(status, &html);
        response.headers_mut().insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("text/html; charset=utf-8"),
        );
        response
    }
}

fn text(status: StatusCode, body: &str) -> Response<Vec<u8>> {
    let mut response = Response::new(body.as_bytes().to_vec());
    *response.status_mut() = status;
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

/// 128 bits imprévisibles sans dépendance : les clés de `RandomState` viennent de l'aléa du
/// système, deux hachages en donnent deux moitiés.
fn random_token() -> String {
    use std::hash::{BuildHasher, Hasher};
    let half = || {
        std::collections::hash_map::RandomState::new()
            .build_hasher()
            .finish()
    };
    format!("{:016x}{:016x}", half(), half())
}

fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

#[cfg(test)]
mod tests;
