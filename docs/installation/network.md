# Réseau et HTTPS

## Sur votre réseau local

Shiori n'écoute que l'appareil où il tourne. Pour le joindre depuis la tablette, faites-le écouter sur le réseau **et** dites-lui par quel nom ou adresse vous l'ouvrez (sinon il refuse la page, par sécurité) :

- programme seul : `shiori serve --listen 0.0.0.0:7878 --allowed-host 192.168.1.10` (l'adresse de l'ordinateur) ;
- Docker : dans `compose.yaml`, `"7878:7878"` au lieu de `"127.0.0.1:7878:7878"`, et `SHIORI_ALLOWED_HOSTS: localhost,192.168.1.10`.

Puis ouvrez `http://192.168.1.10:7878` sur la tablette et connectez-vous.

::: warning
En `http://`, le mot de passe circule en clair sur le réseau : réservez cela à un réseau de confiance. L'application installable et la lecture hors ligne demandent HTTPS.
:::

## Depuis Internet

Mettez un **reverse proxy** devant Shiori : il donne une adresse HTTPS (par exemple `https://shiori.mondomaine.fr`), pour que mots de passe et sessions ne circulent jamais en clair. Exemple avec [Caddy](https://caddyserver.com), qui obtient le certificat tout seul :

```txt
shiori.mondomaine.fr {
    reverse_proxy 127.0.0.1:7878
}
```

Et dites à Shiori ce nom : `--allowed-host shiori.mondomaine.fr` (ou `SHIORI_ALLOWED_HOSTS: localhost,shiori.mondomaine.fr` avec Docker). Nginx Proxy Manager, Traefik ou Cloudflare Tunnel conviennent aussi. Utilisez un **sous-domaine** : Shiori ne fonctionne pas dans un sous-chemin (`mondomaine.fr/shiori`).

Exemple Nginx :

```nginx
server {
    server_name shiori.mondomaine.fr;
    location / {
        proxy_pass http://127.0.0.1:7878;
        proxy_set_header Host $host;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
    }
}
```

Ouvert sur Internet, Shiori se protège seul : connexion obligatoire, pause qui s'allonge après plusieurs mots de passe faux (par compte et par adresse), cookie de session réservé au HTTPS derrière le proxy (`X-Forwarded-Proto`, `X-Forwarded-For`). Si Shiori reste aussi joignable sans passer par le proxy (port `7878` ouvert sur le réseau local), donnez l'adresse du proxy à `--trusted-proxy` (`SHIORI_TRUSTED_PROXIES`) : sinon un appareil du réseau peut s'inventer une adresse et fausser le compte des essais par adresse. Activez la **double authentification** sur les comptes administrateurs. Une protection en plus devant Shiori (mot de passe du proxy, Authelia, Cloudflare Access) reste possible. Détails : [sécurité](/technical/security).

Avec Cloudflare (compte gratuit) : la limite de 100 Mo porte sur chaque requête, et Shiori n'envoie et ne reçoit que des pages de quelques centaines de Ko, y compris pour les tomes téléchargés.

## La même adresse à la maison et dehors

À la maison, inutile de passer par Internet : faites pointer le même nom vers le reverse proxy local avec un **DNS local** (réglage de votre box, Pi-hole, AdGuard Home : `shiori.mondomaine.fr` → l'adresse locale du reverse proxy). Le reverse proxy doit avoir un certificat valable pour ce nom (Caddy ou Let's Encrypt par défi DNS). Vous gardez une seule adresse, donc une seule application installée et les mêmes tomes téléchargés, et à la maison le trafic ne sort pas.

Une application web ne peut pas basculer seule d'une adresse à l'autre : l'adresse est son identité (installation, tomes téléchargés, réglages locaux), et une page HTTPS ne peut pas appeler une adresse `http://192.168…`.
