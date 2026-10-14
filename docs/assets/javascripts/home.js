/* ---------------------------------------------------------------------------
   Home page animation — a schema of entities and their relations, drawn on a
   canvas across the whole hero of `overrides/home.html`.

   Fourteen tables sit as clusters of rows in a slowly turning 3D space that
   fills the viewport, over a field of distant rows that gives it depth. The
   lines between clusters are the foreign keys, and the pulses that run along
   them are queries loading related rows: a parent row lights up, then every
   row that points to it, then theirs, the way a loader walks a relation.
   Everything is drawn with the 2D canvas API and additive compositing, so
   the page needs no library and no build step. The hub `user` sits in the
   middle and the other tables ring it on two ellipses wider than they are
   deep, so the schema spans the whole width of a landscape screen. With
   `prefers-reduced-motion` the schema is drawn once and stands still.
   `home.html` loads this file on the home page only.
   --------------------------------------------------------------------------- */

(function () {
    "use strict";

    /* The Turso Aqua of `brand.css`, as channels for the canvas gradients. */
    var AQUA = [79, 248, 210];

    /* The tables of a blog with accounts: a hub, an inner ring of the
       tables closest to it, an outer ring of the rest, junctions included.
       `rows` is how many rows each cluster shows; `parents` names the tables
       its rows point to. The ring angle, the ring radius and the height
       place the cluster in the world, in radians and world units; `spread`
       is the radius of the cluster. */
    var TABLES = [
        { name: "user", rows: 56, parents: [], angle: 0, radius: 0, height: 0, spread: 0.32 },
        { name: "profile", rows: 40, parents: ["user"], angle: 0.3, radius: 0.75, height: 0.35, spread: 0.24 },
        { name: "post", rows: 80, parents: ["user"], angle: 1.9, radius: 0.75, height: -0.3, spread: 0.36 },
        { name: "role", rows: 16, parents: [], angle: 3.5, radius: 0.75, height: 0.3, spread: 0.16 },
        { name: "tag", rows: 28, parents: [], angle: 5.0, radius: 0.75, height: -0.25, spread: 0.22 },
        { name: "session", rows: 60, parents: ["user"], angle: 0.0, radius: 1.55, height: -0.5, spread: 0.3 },
        { name: "follow", rows: 70, parents: ["user"], angle: 0.75, radius: 1.5, height: 0.55, spread: 0.32 },
        { name: "media", rows: 50, parents: ["post"], angle: 1.45, radius: 1.6, height: -0.4, spread: 0.28 },
        { name: "comment", rows: 110, parents: ["post", "user"], angle: 2.15, radius: 1.5, height: 0.5, spread: 0.42 },
        { name: "like", rows: 90, parents: ["post", "user"], angle: 2.85, radius: 1.6, height: -0.55, spread: 0.36 },
        { name: "user_role", rows: 40, parents: ["user", "role"], angle: 3.6, radius: 1.5, height: 0.45, spread: 0.24 },
        { name: "category", rows: 20, parents: [], angle: 4.3, radius: 1.6, height: -0.35, spread: 0.18 },
        { name: "post_tag", rows: 80, parents: ["post", "tag"], angle: 5.0, radius: 1.5, height: 0.55, spread: 0.34 },
        { name: "post_category", rows: 60, parents: ["post", "category"], angle: 5.65, radius: 1.6, height: -0.45, spread: 0.3 },
    ];

    /* World units: the camera distance, how much wider than deep the ring
       of tables is, the drift of a row around its place, the lean of the
       schema towards the viewer and its base rotation. */
    var CAMERA = 3.2;
    var STRETCH = 1.5;
    var DRIFT = 0.03;
    var TILT = 0.42;
    var SPIN = 0.08;

    /* Distant rows: how many, and the half-size of the box they fill. */
    var FIELD = 900;
    var FIELD_BOX = [4.5, 2.2, 4.5];

    /* Seconds between two queries, the most pulses in flight, how deep a
       query walks the relations and how fast a lit row cools down. */
    var QUERY_EVERY = [0.15, 0.5];
    var PULSE_CAP = 420;
    var QUERY_DEPTH = 3;
    var COOLING = 1.4;

    var reduced = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    var wide = window.matchMedia("(min-width: 76.25em)");

    /* Builds the glow sprite once: a radial falloff in the brand color that
       every row, pulse and halo is a scaled copy of. */
    var sprite = document.createElement("canvas");
    sprite.width = 64;
    sprite.height = 64;
    var sctx = sprite.getContext("2d");
    var gradient = sctx.createRadialGradient(32, 32, 0, 32, 32, 32);
    gradient.addColorStop(0, "rgba(225,255,246,1)");
    gradient.addColorStop(0.12, "rgba(" + AQUA.join(",") + ",0.95)");
    gradient.addColorStop(0.3, "rgba(" + AQUA.join(",") + ",0.5)");
    gradient.addColorStop(0.6, "rgba(" + AQUA.join(",") + ",0.12)");
    gradient.addColorStop(1, "rgba(" + AQUA.join(",") + ",0)");
    sctx.fillStyle = gradient;
    sctx.fillRect(0, 0, 64, 64);

    /* Returns a random point in a sphere, denser towards the center, so a
       cluster reads as a body of rows rather than a shell. */
    function inSphere(radius) {
        var theta = 2 * Math.PI * Math.random();
        var phi = Math.acos(2 * Math.random() - 1);
        var r = radius * Math.cbrt(Math.random());
        return [
            r * Math.sin(phi) * Math.cos(theta),
            r * Math.sin(phi) * Math.sin(theta),
            r * Math.cos(phi),
        ];
    }

    /* The world: one node per row, one edge per foreign key, one point per
       distant row. A node keeps its resting place, a drift phase and its
       heat; an edge keeps its two ends and its own heat; `children` lists
       the edges leaving a parent. */
    var nodes = [];
    var edges = [];
    var clusters = [];
    var field = [];
    var byTable = {};

    TABLES.forEach(function (table) {
        var center = [
            table.radius * STRETCH * Math.cos(table.angle),
            table.height,
            table.radius * Math.sin(table.angle),
        ];
        var rows = [];
        for (var i = 0; i < table.rows; i++) {
            var offset = inSphere(table.spread);
            var node = {
                rest: [center[0] + offset[0], center[1] + offset[1], center[2] + offset[2]],
                pos: [0, 0, 0],
                screen: [0, 0, 0, 0],
                phase: Math.random() * Math.PI * 2,
                heat: 0,
                children: [],
            };
            nodes.push(node);
            rows.push(node);
        }
        byTable[table.name] = rows;
        clusters.push({ table: table, center: center, screen: [0, 0, 0, 0] });
    });

    /* A child row picks its parent with a bias towards the first rows of a
       shuffled list, so a few parents gather many children, as real data
       does, and the queries that start from them fan out visibly. */
    TABLES.forEach(function (table) {
        table.parents.forEach(function (parentName) {
            var parents = byTable[parentName].slice().sort(function () {
                return Math.random() - 0.5;
            });
            byTable[table.name].forEach(function (child) {
                var index = Math.floor(Math.pow(Math.random(), 1.8) * parents.length);
                var edge = { from: parents[index], to: child, heat: 0 };
                edges.push(edge);
                parents[index].children.push(edge);
            });
        });
    });

    for (var f = 0; f < FIELD; f++) {
        field.push({
            rest: [
                (Math.random() * 2 - 1) * FIELD_BOX[0],
                (Math.random() * 2 - 1) * FIELD_BOX[1],
                (Math.random() * 2 - 1) * FIELD_BOX[2],
            ],
            screen: [0, 0, 0, 0],
            size: 0.6 + Math.random() * 1.2,
            twinkle: Math.random() * Math.PI * 2,
        });
    }

    var roots = nodes.filter(function (node) {
        return node.children.length > 0;
    });

    /* Live state: the pulses in flight, the next query time, the hero and
       the canvas in hand with its context and size, and the generation of
       the frame loop. The theme's instant navigation swaps the page without
       reloading it, so the hero is looked up again on every arrival and the
       loop of a hero that left the page stops by its generation. */
    var pulses = [];
    var nextQuery = 0.4;
    var hero = null;
    var canvas = null;
    var ctx = null;
    var size = { width: 0, height: 0, dpr: 1 };
    var generation = 0;
    var last = 0;
    var elapsed = 0;

    /* Starts a query at a parent row: the row flashes, then a pulse leaves
       towards each of its children. Arrivals at `QUERY_DEPTH` stop the walk. */
    function query(root, now, depth) {
        root.heat = 1;
        root.children.forEach(function (edge, i) {
            if (pulses.length >= PULSE_CAP) {
                return;
            }
            pulses.push({
                edge: edge,
                start: now + 0.06 + i * 0.025 + Math.random() * 0.15,
                duration: 0.55 + Math.random() * 0.35,
                depth: depth,
            });
        });
    }

    /* Advances the simulation by `dt` seconds. */
    function step(now, dt) {
        var cooling = Math.exp(-dt * COOLING);
        nodes.forEach(function (node) {
            node.heat *= cooling;
        });
        edges.forEach(function (edge) {
            edge.heat *= cooling;
        });
        if (reduced) {
            return;
        }
        if (now >= nextQuery) {
            query(roots[Math.floor(Math.random() * roots.length)], now, 1);
            nextQuery = now + QUERY_EVERY[0] + Math.random() * (QUERY_EVERY[1] - QUERY_EVERY[0]);
        }
        pulses = pulses.filter(function (pulse) {
            if ((now - pulse.start) / pulse.duration < 1) {
                return true;
            }
            pulse.edge.heat = 1;
            pulse.edge.to.heat = 1;
            if (pulse.depth < QUERY_DEPTH) {
                query(pulse.edge.to, now, pulse.depth + 1);
            }
            return false;
        });
    }

    /* Projects the world onto the canvas: a drift around the resting place,
       a turn around the vertical axis, a lean towards the viewer that
       breathes slowly, then the perspective. `screen` holds x, y, the
       scale and the depth in [0, 1]. Returns the pixel size of a world unit. */
    function project(now) {
        var spin = reduced ? 0.9 : now * SPIN;
        var lean = TILT + (reduced ? 0 : 0.08 * Math.sin(now * 0.15));
        var cosS = Math.cos(spin);
        var sinS = Math.sin(spin);
        var cosL = Math.cos(lean);
        var sinL = Math.sin(lean);
        var cx = size.width * 0.5;
        var cy = wide.matches ? size.height * 0.5 : size.height * 0.45;
        var unit = wide.matches
            ? Math.max(size.width * 0.21, size.height * 0.36)
            : Math.max(size.width * 0.32, size.height * 0.32);

        function place(p, out) {
            var x = p[0] * cosS - p[2] * sinS;
            var z = p[0] * sinS + p[2] * cosS;
            var y = p[1] * cosL - z * sinL;
            z = p[1] * sinL + z * cosL;
            var scale = CAMERA / (CAMERA - Math.min(z, CAMERA - 0.4));
            out[0] = cx + x * unit * scale;
            out[1] = cy - y * unit * scale;
            out[2] = scale;
            out[3] = Math.min(1, Math.max(0, (z + 2.0) / 4.0));
        }

        var drift = reduced ? 0 : DRIFT;
        nodes.forEach(function (node) {
            node.pos[0] = node.rest[0] + drift * Math.sin(now * 0.7 + node.phase);
            node.pos[1] = node.rest[1] + drift * Math.cos(now * 0.5 + node.phase * 1.3);
            node.pos[2] = node.rest[2] + drift * Math.sin(now * 0.6 + node.phase * 0.7);
            place(node.pos, node.screen);
        });
        clusters.forEach(function (cluster) {
            place(cluster.center, cluster.screen);
        });
        field.forEach(function (point) {
            place(point.rest, point.screen);
        });
        return unit;
    }

    /* Draws one glow of the sprite, `radius` pixels wide, at the given alpha. */
    function glow(x, y, radius, alpha) {
        ctx.globalAlpha = alpha;
        ctx.drawImage(sprite, x - radius, y - radius, radius * 2, radius * 2);
    }

    /* Draws a frame. The field, a soft halo per table, the edges, the rows
       and the pulses add light onto the transparent canvas. */
    function draw(now, unit) {
        var aqua = AQUA.join(",");
        ctx.clearRect(0, 0, size.width, size.height);
        ctx.globalCompositeOperation = "lighter";

        ctx.fillStyle = "rgb(" + aqua + ")";
        field.forEach(function (point) {
            var s = point.screen;
            if (s[0] < -4 || s[0] > size.width + 4 || s[1] < -4 || s[1] > size.height + 4) {
                return;
            }
            var blink = reduced ? 1 : 0.75 + 0.25 * Math.sin(now * 1.3 + point.twinkle);
            ctx.globalAlpha = (0.16 + 0.5 * s[3]) * blink;
            ctx.beginPath();
            ctx.arc(s[0], s[1], Math.min(2.2, point.size * s[2]) / 2, 0, Math.PI * 2);
            ctx.fill();
        });

        clusters.forEach(function (cluster) {
            var s = cluster.screen;
            glow(s[0], s[1], cluster.table.spread * unit * s[2] * 1.8, 0.14 + 0.2 * s[3]);
        });

        edges.forEach(function (edge) {
            var a = edge.from.screen;
            var b = edge.to.screen;
            var depth = (a[3] + b[3]) / 2;
            var alpha = (0.22 + 0.7 * edge.heat) * (0.3 + 0.7 * depth);
            ctx.lineWidth = 1 + 1.5 * edge.heat;
            ctx.strokeStyle = "rgba(" + aqua + "," + alpha.toFixed(3) + ")";
            ctx.beginPath();
            ctx.moveTo(a[0], a[1]);
            ctx.lineTo(b[0], b[1]);
            ctx.stroke();
        });

        nodes.forEach(function (node) {
            var s = node.screen;
            var fog = 0.35 + 0.65 * s[3];
            var radius = (unit * 0.013 + 1.6) * s[2] * (1 + 2.2 * node.heat);
            glow(s[0], s[1], radius * 3.2, (0.5 + 0.5 * node.heat) * fog);
            ctx.globalAlpha = (0.8 + 0.2 * node.heat) * fog;
            ctx.fillStyle = node.heat > 0.4 ? "#ffffff" : "rgb(" + aqua + ")";
            ctx.beginPath();
            ctx.arc(s[0], s[1], radius * 0.4, 0, Math.PI * 2);
            ctx.fill();
        });

        pulses.forEach(function (pulse) {
            var t = (now - pulse.start) / pulse.duration;
            if (t < 0) {
                return;
            }
            var a = pulse.edge.from.screen;
            var b = pulse.edge.to.screen;
            var eased = t * t * (3 - 2 * t);
            for (var k = 0; k < 8; k++) {
                var u = Math.max(0, eased - k * 0.03);
                var x = a[0] + (b[0] - a[0]) * u;
                var y = a[1] + (b[1] - a[1]) * u;
                var scale = a[2] + (b[2] - a[2]) * u;
                glow(x, y, (unit * 0.04 + 3) * scale * (1 - k * 0.1), 1 - k * 0.11);
            }
        });

        ctx.globalCompositeOperation = "source-over";
        ctx.globalAlpha = 1;
    }

    /* Fits the canvas to the hero at the device pixel ratio, capped at 2. */
    function resize() {
        var rect = hero.getBoundingClientRect();
        size.width = Math.max(1, Math.round(rect.width));
        size.height = Math.max(1, Math.round(rect.height));
        size.dpr = Math.min(2, window.devicePixelRatio || 1);
        canvas.width = size.width * size.dpr;
        canvas.height = size.height * size.dpr;
        ctx.setTransform(size.dpr, 0, 0, size.dpr, 0, 0);
    }

    /* Takes the hero of the current page, if any, and runs the loop on its
       canvas. A page without a hero, or whose hero is already in hand,
       changes nothing. The loop keeps its generation and stops as soon as a
       later mount has moved on; the time step is clamped so a tab coming
       back from the background does not jump. */
    function mount() {
        var next = document.querySelector(".tx-hero__canvas");
        if (next === canvas) {
            return;
        }
        generation += 1;
        window.removeEventListener("resize", resize);
        canvas = next;
        hero = next ? next.parentNode : null;
        ctx = next && next.getContext ? next.getContext("2d") : null;
        if (!hero || !ctx) {
            canvas = null;
            return;
        }
        resize();
        window.addEventListener("resize", resize);
        last = 0;
        var token = generation;
        window.requestAnimationFrame(function frame(timestamp) {
            if (token !== generation) {
                return;
            }
            var dt = last ? Math.min(0.05, (timestamp - last) / 1000) : 0;
            last = timestamp;
            elapsed += dt;
            step(elapsed, dt);
            draw(elapsed, project(elapsed));
            if (!reduced) {
                window.requestAnimationFrame(frame);
            }
        });
    }

    /* `document$` is the theme's observable of the current document: it
       emits once now and again after every instant navigation. */
    if (window.document$ && typeof window.document$.subscribe === "function") {
        window.document$.subscribe(mount);
    } else {
        mount();
    }
})();
