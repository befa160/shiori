# En cas de problème

| Symptôme | Que faire |
|---|---|
| **Mot de passe oublié** | L'administrateur en donne un nouveau dans **Administration → Comptes**. Pour l'administrateur lui-même : `shiori user password <nom>` (Docker : `docker compose exec shiori shiori user password <nom>`), qui ferme aussi ses sessions. |
| **Téléphone de la double authentification perdu** | Un **code de secours** (donnés à l'activation, chacun sert une fois). Sinon, un administrateur la retire (**Administration → Comptes**, ou `shiori user totp-reset <nom>`). |
| **« Trop d'essais »** | Après plusieurs mots de passe faux, Shiori fait patienter, de plus en plus longtemps (15 minutes au plus). |
| **Jeton d'installation introuvable** | Il n'est demandé que tant qu'aucun compte n'existe ; il s'affiche à chaque démarrage dans la fenêtre de Shiori ou dans `docker compose logs shiori`. |
| **« nom d'hôte non autorisé »** | Vous ouvrez Shiori par un nom ou une adresse qu'il ne connaît pas : ajoutez-le à `--allowed-host` / `SHIORI_ALLOWED_HOSTS`. |
| **Docker, « unable to open database file »** | Un dossier de l'hôte monté sur `/data` appartient à root. Gardez les volumes nommés du `compose.yaml`, ou donnez ce dossier à l'utilisateur du conteneur (`user:`). |
| **Une série manque ou est mal découpée** | Voir [Ranger ses fichiers](/guide/libraries#ranger-ses-fichiers) et le journal du scan (**Administration → Historique et options**). |
| **Un tome est en erreur** | Son menu → **Problèmes du scan**, ou **Administration → Santé des bibliothèques** ; chaque code est expliqué dans [codes de diagnostic](/guide/diagnostics). |
| **Un fichier ajouté n'apparaît pas** | La surveillance attend 30 s de calme après le dernier changement. Sur un partage réseau ou avec Docker Desktop, les changements faits ailleurs ne sont pas signalés : lancez un scan, ou attendez le scan complet périodique. |
| **Le scan échoue « dossier démonté ? »** | Le dossier de la bibliothèque est vide ou illisible alors que la base y connaît des fichiers : Shiori refuse de tout retirer. Remontez le disque ou le partage, puis relancez. |
| **L'application ne s'installe pas, pas de hors-ligne** | Il faut une adresse HTTPS (ou `localhost`) : voir [Réseau et HTTPS](/installation/network). |
| **Linux : « ne peut pas être surveillé »** au journal | Trop de dossiers pour la limite d'inotify : augmentez `fs.inotify.max_user_watches` (`sysctl`). Le scan complet périodique continue de toute façon. |
