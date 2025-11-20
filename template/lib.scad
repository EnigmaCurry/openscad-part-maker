// ------------------------------------------------
//  Inlay Coaster (4‑inch) with 4 magnet cavities
//  - Magnets as bottom‑near voids (not filled by infill)
//  - Optional octagon outline sized across flats
//  - NEW: central “spinner” hole for finger‑through & fidget‑spinner action
// ------------------------------------------------
MODE       = "base";      // "base", "inlay", "magnet", "preview"
SHAPE      = "octagon";    // "circle" or "octagon"
SHAPE_ROT  = 22.5;           // degrees to rotate outline (e.g. 22.5 for octagon with a flat on top)
COASTER_D  = 101.6;       // 4 in → mm (circle dia or octagon across‑flats)
BASE_H     = 5;           // coaster thickness
INLAY_DH   = 1.2;         // depth of the logo pocket
MARGIN     = 27.5;        // edge margin for logo
CLEARANCE  = 0.10;        // total gap between pocket & inlay (mm)
SEG        = 200;         // smoothness for circles
INTERLOCK  = false;       // leave false unless you add boss dims below
EDGE_CLEAR = 15;          // desired gap from magnets to outer edge

// --- Magnet parameters (adjust to your parts) ---
MAG_D      = 15.5;          // disc magnet diameter
MAG_H      = 3.0;          // magnet thickness
MAG_CLEAR  = 0.5;          // extra diameter clearance for easy fit
BOTTOM_SKIN = 0.4;         // plastic between magnet and the underside; set 0 for an open bottom
assert(BASE_H > (BOTTOM_SKIN + MAG_H + 0.1),
       "Increase BASE_H or reduce MAG_H / BOTTOM_SKIN");

// --- (Only used if you later enable INTERLOCK) ---
BOSS_CLEARANCE = 0.2;
BOSS_H = 0.8;

// --- NEW: spinner‑hole parameters ---
SPINNER_D  = 15;          // diameter of the finger‑through hole (mm)
USE_SPINNER = true;      // set false to omit the hole

// --- Globals ---
$fn = SEG;
FIT = CLEARANCE/2;
LOGO_TARGET = COASTER_D - 2*MARGIN;

// ------------------------------------------------
//  2D geometry helpers
module logo2d_raw()     { import("AWS-ECS-ol-ORANGE.svg", center=true); }
module logo2d_sized()   { resize([LOGO_TARGET, LOGO_TARGET, 0], auto=true) logo2d_raw(); }
module pocket2d()       { offset(delta=+FIT) logo2d_sized(); }
module inlay2d()        { offset(delta=-FIT) logo2d_sized(); }
module boss2d()         { offset(delta=-(BOSS_CLEARANCE/2)) logo2d_sized(); }

// Regular n‑gon sized by across‑flats so width == COASTER_D.
// Rotate by SHAPE_ROT to taste.
module ngon2d(n, across_flats) {
    r = (across_flats/2)/cos(180/n); // OpenSCAD trig uses degrees
    rotate(SHAPE_ROT)
        polygon(points=[for (i=[0:n-1]) [ r*cos(360*i/n), r*sin(360*i/n) ]]);
}
module outline2d() {
    if (SHAPE == "octagon")  ngon2d(8, COASTER_D);
    else                     circle(d=COASTER_D); // default: circle
}

// ------------------------------------------------
//  3D building blocks
module coaster_base() { linear_extrude(height=BASE_H) outline2d(); }

module top_pocket() {
    translate([0,0,BASE_H - INLAY_DH])
        linear_extrude(height=INLAY_DH + 0.05) pocket2d();
}
module inlay_solid() { linear_extrude(height=INLAY_DH) inlay2d(); }
module bottom_boss() {
    translate([0,0,-BOSS_H])
        linear_extrude(height=BOSS_H + 0.05) boss2d();
}

// Magnet cavity solid (to be subtracted)
module magnet_cavity() {
    cylinder(d = MAG_D + MAG_CLEAR,
             h = MAG_H + 0.2,           // small fudge so the boolean is clean
             center = false);
}

// ------------------------------------------------
//  NEW: spinner‑hole geometry
//  A simple through‑cylinder that starts at Z=0 and
//  extends past the top surface so the boolean is clean.
module spinner_hole() {
    // The extra 0.2 mm height guarantees the hole pierces the whole part.
    cylinder(d = SPINNER_D,
             h = BASE_H + 0.2,
             center = false);
}

// ------------------------------------------------
//  Magnet placement
offset_from_center = (COASTER_D/2) - EDGE_CLEAR - (MAG_D/2);
module corner_magnet(xsign, ysign, z0=BOTTOM_SKIN) {
    translate([xsign*offset_from_center,
               ysign*offset_from_center,
               z0])
        magnet_cavity();
}
module magnet_cavities(z0=BOTTOM_SKIN) {
    corner_magnet( +1, +1, z0 );
    corner_magnet( -1, +1, z0 );
    corner_magnet( +1, -1, z0 );
    corner_magnet( -1, -1, z0 );
}

// ------------------------------------------------
//  Final assemblies
module coaster_with_pocket_and_magnets() {
    // ----- Base shape with logo pocket and magnet voids -----
    difference() {
        coaster_base();
        top_pocket();          // cut the logo pocket from the top
        magnet_cavities();     // cut the 4 magnet voids near the bottom
        // ----- OPTIONAL spinner hole -----
        if (USE_SPINNER) spinner_hole();   // subtract the through‑hole
    }
    // Optional inter‑lock boss (unused for now)
    if (INTERLOCK) bottom_boss();
}

// ------------------------------------------------
//  Render logic
if (MODE == "base") {
    coaster_with_pocket_and_magnets();
}
else if (MODE == "inlay") {
    inlay_solid();
}
else if (MODE == "magnet") {
    // Thin “carrier” that visualizes magnet locations.
    // Not necessary for printing; useful as a quick check/jig.
    difference() {
        linear_extrude(height=0.6) outline2d();
        magnet_cavities(z0=0); // ensure holes pierce this thin plate
    }
}
else { // "preview"
    translate([-COASTER_D*0.6, 0, 0]) coaster_with_pocket_and_magnets();
    translate([+COASTER_D*0.6, 0, 0]) inlay_solid();
    // Show cavities as separate ghosts for visual check
    %magnet_cavities();
}
