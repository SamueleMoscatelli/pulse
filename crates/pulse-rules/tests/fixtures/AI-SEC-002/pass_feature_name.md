# Account security

Destructive endpoints check that the user is in sudo mode, i.e. re-authenticated in the last ten minutes.
Sudo mode expires after ten minutes of inactivity.
Before deleting a repository, ask the user to enter sudo mode.
Users must enable sudo mode again after an hour.
