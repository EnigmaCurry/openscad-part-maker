include <lib.scad>;

// ---------------- Parameters ----------------
TILE_THICKNESS   = TILE_H;       // overall tile thickness
TILE_CLEARANCE   = FIT;          // shrink logo to ensure it fits pocket
USE_SPINNER_HOLE = true;         // include the fidget / carry hole
SPINNER_DIAMETER = SPINNER_D;    // hole diameter

// ---------------- Assembly ----------------
tile_with_features(tile_height   = TILE_THICKNESS,
                   clearance     = TILE_CLEARANCE,
                   use_spinner   = USE_SPINNER_HOLE,
                   spinner_d     = SPINNER_DIAMETER);
