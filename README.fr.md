# AudioBridge — application Windows

Récepteur audio Bluetooth local écrit en Rust. **Double-cliquer sur AudioBridge.exe, puis faire un clic droit sur son icône près de l'horloge.** Si elle n'est pas visible, ouvrir la petite flèche des icônes masquées.

## Utilisation

![Menu natif AudioBridge réellement exécuté](assets/screenshots/tray-menu.png)

1. Appairer le téléphone dans les paramètres Bluetooth Windows.
2. Quitter les autres récepteurs Bluetooth avant l'essai.
3. Ouvrir AudioBridge, clic droit sur l'icône, actualiser les appareils puis choisir le téléphone.
4. Sélectionner le PC comme sortie multimédia sur le téléphone et lancer la lecture.

Le menu permet de connecter un appareil, déconnecter, rétablir la connexion, activer/désactiver la récupération automatique, produire un diagnostic, ouvrir les journaux, accéder aux paramètres Bluetooth, afficher À propos et quitter.

Aucune console n'est ouverte par l'application principale. Les opérations Bluetooth s'exécutent sur un thread séparé du menu. Quitter peut attendre la fin d'une opération Windows en cours, puis ferme la session. Il n'y a ni service, ni démarrage automatique, ni compte, ni télémétrie.

**Version communautaire préliminaire 0.3, pas une garantie de stabilité universelle.** La récupération est désactivée à chaque lancement. Si activée : trois reprises au maximum espacées de 2/4/8 secondes par demande explicite. Pas de reconnexion déclenchée sur la seule base du silence.

## Si la connexion est silencieuse

Ne pas multiplier immédiatement les réparations. Laisser le média jouer et lancer le diagnostic depuis le menu. Si le son revient seul, refaire un diagnostic dans l'état fonctionnel pour comparer. Les rapports sont locaux et peuvent contenir les noms/identifiants des appareils : relire avant partage.

« Connecté » ne prouve pas la présence de son. Le filtre A2DP SNK peut trouver zéro candidat alors que la lecture fonctionne. Un niveau nul ou une mesure indisponible ne démontre pas une panne ; le pic de la sortie peut venir d'une autre application. Voir [DIAGNOSTICS.md](DIAGNOSTICS.md).

## Compatibilité et installation

Package portable Windows x64, sans droits administrateur pour l'usage normal. API disponible depuis Windows 10 version 2004 ; essais locaux sur Windows 11 x64 et Android seulement. La compatibilité iPhone n'est pas vérifiée localement. Pas de gestion du microphone/des appels, de choix de sortie audio, d'installateur ou de mise à jour automatique.

Le runtime Microsoft Visual C++ v14 x64 doit être présent : le binaire MSVC inspecté dépend de `VCRUNTIME140.dll`. S'il manque, utiliser uniquement le [redistribuable officiel Microsoft](https://learn.microsoft.com/en-us/cpp/windows/latest-supported-vc-redist?view=msvc-170), jamais un téléchargement de DLL isolée. Le package portable ne l'installe pas automatiquement.

Binaire de développement non signé : le projet ne demande pas de désactiver les protections Windows ou d'installer un certificat. Aucun pilote n'est fourni ou modifié.

La version console optionnelle s'appelle AudioBridge-CLI.exe. Pour désinstaller : quitter, supprimer le dossier et éventuellement `%LOCALAPPDATA%\AudioBridgeRust` contenant journaux/rapports.

Voir README.md pour construire, CONTRIBUTING.md pour tester et PUBLISHING.md pour préparer la publication. Licence MIT ; notices des dépendances incluses. Les workflows GitHub sont préparés, mais leur succès ne peut être affirmé avant exécution sur le dépôt distant.

## Annulation, erreurs et journaux

« Déconnecter » reste disponible pendant le délai de récupération et une opération en cours. L’annulation est mise en file : elle attend le retour de l’appel Windows et passe avant la prochaine récupération automatique. La préférence de récupération reste inchangée.

Une lecture d’état Windows en échec retire immédiatement la confirmation de connexion. Trois contrôles périodiques consécutifs en échec entraînent une fermeture, puis une récupération uniquement si elle est activée et toujours souhaitée. Un échec de fermeture conserve la session et bloque les opérations automatiques jusqu’à une action explicite. Un simple contrôle ou une action sans rapport n’efface pas l’erreur.

Pendant un menu ou dialogue modal, seul le dernier instantané est conservé ; erreurs, rapports et arrêt restent en file et sont traités au retour du dialogue. Les contrôles stables n’écrivent plus de traces et ne modifient plus l’infobulle. Une mise à jour d’infobulle échouée est réessayée de façon temporisée jusqu’à application, sans répéter les mêmes erreurs dans le journal.

Les journaux visent une limite totale de 7 Mio : la session active et quatre autres fichiers de session, plus `tray.log` avec un segment précédent, chaque segment écrit faisant au maximum 1 Mio. La session active est protégée du nettoyage, même après un recul de l’horloge. Les anciens segments de session sont écrasés et les sessions les plus anciennes supprimées au lancement. Les anciens journaux reconnus trop volumineux sont supprimés lors de cette migration. Si le nettoyage est bloqué, il est signalé sans empêcher le démarrage audio ; la limite totale ne peut alors être garantie avant un nettoyage réussi. Les rapports de diagnostic et fichiers étrangers sont exclus de cette rétention. En cas d’échec d’écriture, un seul signalement est produit puis ce journal est désactivé jusqu’au redémarrage.
