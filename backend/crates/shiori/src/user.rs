//! `shiori user` : comptes gérés sans interface, sur la base locale (premier administrateur
//! sans page d'installation, mot de passe oublié).

use std::path::PathBuf;

use anyhow::Result;
use clap::{Args, Subcommand};
use shiori_db::{Role, temporary_password};

#[derive(Args)]
pub struct UserArgs {
    /// Base SQLite (défaut : <dossier de données>/shiori.db, ou $SHIORI_DATA_DIR/shiori.db).
    #[arg(long, value_name = "FICHIER", global = true)]
    db: Option<PathBuf>,
    #[command(subcommand)]
    command: UserCommand,
}

#[derive(Subcommand)]
enum UserCommand {
    /// Crée un compte (lecteur par défaut) avec un mot de passe provisoire, affiché une fois.
    Add {
        name: String,
        /// Compte administrateur.
        #[arg(long)]
        admin: bool,
    },
    /// Donne un nouveau mot de passe provisoire et ferme toutes les sessions du compte.
    Password { name: String },
}

pub async fn run(args: UserArgs) -> Result<()> {
    let db = crate::open_db(args.db).await?;
    let password = temporary_password();
    match args.command {
        UserCommand::Add { name, admin } => {
            let role = if admin { Role::Admin } else { Role::Reader };
            let user = db.create_user(&name, &password, role).await?;
            let role = if admin { "administrateur" } else { "lecteur" };
            println!("Compte « {} » créé ({role}).", user.username);
        }
        UserCommand::Password { name } => {
            db.set_password(&name, &password, true).await?;
            println!("Mot de passe de « {name} » remplacé, sessions fermées.");
        }
    }
    println!("Mot de passe provisoire : {password}");
    println!("À changer à la première connexion.");
    Ok(())
}
