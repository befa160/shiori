-- Thème de l'interface (lot 6.4, Q40) : `system` suit le réglage de l'appareil (sombre ou
-- clair), les autres l'imposent. Le lecteur reste sombre quel que soit le thème.
ALTER TABLE app_settings ADD COLUMN theme TEXT NOT NULL DEFAULT 'system'
  CHECK (theme IN ('system','dark','light','oled','sepia'));
