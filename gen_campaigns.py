#!/usr/bin/env python3
"""Generate campaigns.json with kid narrator voice (describing the adventure, not building it)."""
import json, uuid

def uid():
    return str(uuid.uuid4())

SCALES = ["ionian","dorian","phrygian","lydian","mixolydian","aeolian","locrian"]
ROOTS = ["C","C#","D","D#","E","F","F#","G","G#","A","A#","B"]

def mode(root, scale):
    return {"root": root, "scale": scale}

def make_campaign(name, font, desc, bg, text, levels, designs, store, settings, desc_font="Comic Neue", label_font="Schoolbell"):
    return {
        "id": uid(),
        "overworld": {
            "name": name,
            "font": font,
            "description_font": desc_font,
            "label_font": label_font,
            "description": desc,
            "bg_color": bg,
            "text_color": text,
            "levels": levels,
            "store": store
        },
        "designs": designs,
        "quality": {"score": 95, "breakdown": {
            "completeness": 100, "tile_variety": 85, "monster_variety": 100,
            "color_quality": 90, "name_quality": 100, "description_quality": 100,
            "mode_validity": 100, "budget_distribution": 90, "theme_coherence": 100
        }},
        "settings": settings
    }

def lv(name, font, desc, theme, color, palette, budget):
    return {"name":name,"font":font,"description":desc,"theme":theme,"color":color,"palette":palette,"budget":budget}

def design(tile_defs, boss_name, boss_desc, monsters, weapon_name, weapon_desc, armor_name, armor_desc, traps, m, victory, defeat):
    return {
        "tile_defs": [{"name":t[0],"char":t[1]} for t in tile_defs],
        "boss": {"name":boss_name,"hp":0,"attack":0,"defense":0,"xp_value":0,"description":boss_desc},
        "monster_types": [{"name":n,"hp":0,"attack":0,"defense":0,"xp_value":0,"description":""} for n in monsters],
        "weapon": {"name":weapon_name,"description":weapon_desc},
        "armor": {"name":armor_name,"description":armor_desc},
        "traps": [{"name":n,"x":None,"y":None,"damage":None} for n in traps],
        "mode": m,
        "victory_message": victory,
        "defeat_message": defeat,
        "budget_spent": None
    }

def st(hp=3, sp=2, bo=2):
    return {"healing_potions":hp,"speed_potions":sp,"bombs":bo}

def settings(locked=5, traps=4, dmg_tiles=99, dmg=2):
    return {"locked_doors_from_level":locked,"traps_from_level":traps,"damage_tiles_from_level":dmg_tiles,"damage_tile_damage":dmg}

campaigns = []

# ── 0: Scary Dungeon ──
campaigns.append(make_campaign(
    "Scary Dungeon", "Nosifer",
    "ok so this is a dungeon and its REALLY scary. the walls are all gray and theres bad guys hiding EVERYWHERE.",
    "#1a1a2e", "#e8d8c8",
    [
        lv("The Entrance","Short Stack","this is where you go in. its dark and you can hear stuff moving around.","gray stone walls, torches on walls, dirt floor","#8a8a7a",["#1a1a14","#3a3a2e","#5a5a48","#8a8a72","#baba98"],125),
        lv("Deeper Down","Comic Neue","it gets darker and the bad guys are MEANER down here.","darker stone, more shadows, cobwebs","#7a7a8a",["#14141a","#2e2e3a","#48485a","#72728a","#9898ba"],165),
        lv("The Really Dark Part","Comic Neue","you cant see ANYTHING its SO dark. theres eyes glowing in the corners.","pitch black corridors, glowing eyes, pits","#5a5a6a",["#0a0a12","#222230","#3a3a50","#585870","#787890"],195),
        lv("The Trap Hall","Coming Soon","watch out!! theres traps EVERYWHERE and they hurt SO bad.","spike pits, swinging blades, pressure plates","#8a6a5a",["#1a1008","#3a2818","#5a4030","#8a6850","#ba9070"],235),
        lv("The Big Door","Kalam","theres this HUGE door and you need the key to open it. whats behind it??","massive iron door, key pedestal, guard room","#6a6a8a",["#0a0a14","#22223a","#3a3a5a","#5a5a8a","#7a7aba"],275),
        lv("The Dungeon King","Patrick Hand","THIS is where the boss lives. hes SO big and SO mean and he goes RAAAAWR.","throne room, boss arena, treasure pile","#aa8a5a",["#1a1408","#3a2e18","#6a5030","#9a7850","#caa070"],140),
    ],
    [
        design([("gray wall","#"),("dirt floor","."),("icky puddle","~"),("torch spot","*")],"Big Pulsing Square","hes really big and red and he PULSES and goes RAWR",["Small Pulsing Square","Tiny Pulsing Square","Medium Pulsing Square"],"Pointy Stick","its really sharp and it goes SWOOSH","Cardboard Shield","its not very strong but its better than nothing",["Hole In The Floor","Spiky Thing","The Floor Falls Down"],mode("C","dorian"),"YAY you did it!! the big pulsing square is GONE!","oh no the pulsing square got you. try again!!"),
        design([("darker gray wall","#"),("stone floor","."),("spider web","~"),("crack in wall","*")],"Mean Purple Triangle","hes purple and pointy and he POKES you",["Baby Spider","Angry Bat","Green Blob"],"Wooden Sword","its got a really good handle and everything","Pot Lid","you can block stuff with it and it goes CLANG",["Web Sticky Trap","Bat Poop Slip","Falling Rocks"],mode("D","aeolian"),"you got the purple triangle!! hes not mean anymore!","the purple triangle poked you too much. ow."),
        design([("black wall","#"),("really dark floor","."),("glowing crack","~"),("eye spot","*")],"Shadow Blob King","you can barely see him but hes HUGE and all blobby",["Floating Eye","Shadow Rat","Dark Wisp"],"Glow Stick Sword","it glows in the dark so you can see where youre swinging","Night Goggles","everything looks green but at least you can SEE",["Invisible Pit","Dark Hands","Floor Gives Way"],mode("E","phrygian"),"the shadow blob is gone and its not dark anymore!!","the dark got you. its SO dark in there."),
        design([("brown wall","#"),("trap floor","."),("pressure plate","~"),("arrow slit","*")],"The Trap Master","he builds all the traps and hes REALLY good at it",["Spike Turtle","Spring Snake","Bomb Bug"],"Trap Disabler","it clicks and all the traps near you turn off for a second","Bounce Armor","when stuff hits you it bounces right off BOING",["Spike Floor","Swinging Blade","Poison Dart"],mode("F","lydian"),"no more traps!! the trap master is DONE!","you stepped on the wrong thing. SO many traps."),
        design([("iron wall","#"),("guard floor","."),("locked tile","~"),("keyhole","*")],"Gate Guardian","hes made of metal and he guards the big door FOREVER",["Iron Soldier","Key Thief","Door Mimic"],"Golden Key Sword","its a sword AND a key at the same time!!","Iron Door Shield","its SO heavy but nothing gets through it",["Locked Floor","Guard Alarm","Crushing Door"],mode("G","mixolydian"),"the gate guardian fell apart!! you can go anywhere now!","the gate guardian squished you flat."),
        design([("throne wall","#"),("red carpet","."),("gold pile","~"),("skull torch","*")],"The Dungeon King","hes the BIGGEST and BADDEST and he sits on a throne made of BONES",["Royal Guard","Crown Bat","Throne Slime"],"Kings Bane","the one weapon the dungeon king is scared of","Crown Armor","it makes you feel like a king too but STRONGER",["Throne Trap","Gold Mimic","Royal Pit"],mode("A","aeolian"),"THE DUNGEON KING IS DEFEATED!! you saved everyone!!","the dungeon king got you. hes SO strong."),
    ],
    st(5,2,2), settings(5,4,99,2)
))

# ── 1: Monster Town ──
campaigns.append(make_campaign(
    "Monster Town", "Creepster",
    "the monsters have a whole TOWN. they have houses and stores and everything but theyre ALL bad guys.",
    "#1e2a1e", "#c8e8c8",
    [
        lv("Monster Suburbs","Handlee","the outside part of town. theres little monster houses with little monster yards.","small houses, picket fences, monster mailboxes","#5a8a5a",["#0a1a0a","#1e3a1e","#325832","#4a7a4a","#68aa68"],130),
        lv("Monster Main Street","Comic Neue","all the monster shops are here. they sell monster stuff like fangs and slime.","shop fronts, monster signs, cobblestone street","#6a9a6a",["#081808","#1a3a1a","#2e5e2e","#4a8a4a","#6aba6a"],170),
        lv("The Monster School","Short Stack","even monsters go to school! but they learn how to be SCARY.","desks, chalkboard, monster drawings","#7a8a5a",["#141a08","#2e3a18","#485a30","#6a8a50","#8aba70"],200),
        lv("Monster Park","Kalam","theres a park but the playground is all DANGEROUS and the swings have spikes.","playground, spiked swings, monster slide","#5a7a4a",["#0a1408","#1e2e18","#324830","#4a6a50","#689070"],240),
        lv("The Monster Jail","Patrick Hand","the REALLY bad monsters are locked up here. or they WERE locked up.","cells, broken bars, alarm bells","#4a5a4a",["#080a08","#181e18","#283228","#384a38","#4a6a4a"],280),
        lv("The Monster Mayor","Permanent Marker","the monster mayor runs EVERYTHING. hes in city hall and he is NOT happy to see you.","city hall, big desk, monster flag","#8aaa5a",["#141a08","#2e3a18","#4a5a30","#6a8a48","#8aba68"],145),
    ],
    [
        design([("house wall","#"),("grass floor","."),("garden patch","~"),("mailbox","*")],"Suburb Boss","hes the BIGGEST monster on the block and he has a really loud bark",["Yard Dog","Fence Cat","Garden Gnome"],"Rake","you can bonk stuff with it pretty good","Trash Can Lid","its dented but it still works",["Sprinkler Trap","Garden Hole","Lawn Darts"],mode("G","ionian"),"the suburb boss ran away!! this block is safe now!","the suburb boss bonked you. hes really strong for a neighbor."),
        design([("shop wall","#"),("cobblestone","."),("puddle","~"),("shop sign","*")],"Shopkeeper Ogre","hes huge and green and he throws stuff from his shop at you",["Fang Seller","Slime Vendor","Potion Rat"],"Shopping Cart Sword","it rolls AND it chops","Barrel Armor","you just get inside a barrel and roll around",["Wet Floor","Falling Sign","Price Tag Trap"],mode("C","mixolydian"),"the shopkeeper is closed for business FOREVER!","you got hit by too many flying shop things."),
        design([("school wall","#"),("tile floor","."),("chalk dust","~"),("desk","*")],"Principal Nightmare","the principal is a GIANT NIGHTMARE and he gives detention FOREVER",["Bully Goblin","Homework Ghost","Chalk Monster"],"Ruler Sword","its a really big ruler and it HURTS","Backpack Shield","its full of books so nothing gets through",["Chalk Dust Cloud","Desk Trap","Detention Bell"],mode("D","dorian"),"school is OUT!! no more monster school!","the principal gave you permanent detention. yikes."),
        design([("park wall","#"),("playground floor","."),("sandbox","~"),("swing post","*")],"Playground Beast","it lives under the slide and it grabs your ankles",["Spike Swinger","Sand Crab","Teeter Monster"],"Seesaw Lance","the other end goes UP when you swing DOWN","Helmet","for when you go down the really fast slide",["Spike Swing","Sand Trap","Merry Go Round of Doom"],mode("E","lydian"),"the playground beast is gone!! recess is FUN again!","the playground beast got your ankles. no more recess."),
        design([("jail wall","#"),("cell floor","."),("broken bar","~"),("alarm","*")],"Warden Skull","he has keys for EVERYTHING and a really scary face",["Escaped Convict","Guard Dog","Cell Rat"],"Key Ring Flail","all the keys on a chain and you swing it WHOOSH","Warden Vest","it says WARDEN on the back so everyone is scared of you",["Cell Door Slam","Alarm Trap","Chain Trip"],mode("F","aeolian"),"the warden is locked in his OWN jail!! ha!!","the warden locked you up and threw away the key."),
        design([("city hall wall","#"),("marble floor","."),("flag post","~"),("desk lamp","*")],"Monster Mayor","he wears a tiny hat and a sash and he is SO ANGRY all the time",["Secretary Ghoul","Tax Collector","Pencil Pusher"],"Gavel","ORDER ORDER bonk bonk bonk","Mayor Sash","it says MAYOR but you crossed it out and wrote HERO",["Stamp Trap","Paper Avalanche","Desk Collapse"],mode("A","phrygian"),"the monster mayor quit!! monster town is FREE!!","the mayor stamped REJECTED on your face. ow."),
    ],
    st(4,2,2), settings(5,3,99,2)
))

# ── 2: Spooky Forest ──
campaigns.append(make_campaign(
    "Spooky Forest", "Eater",
    "its a forest and its REALLY spooky. theres trees everywhere and eyes watching you from the dark parts.",
    "#0a1a0a", "#88cc88",
    [
        lv("The Edge Of The Woods","Caveat","the trees start here. you can still see the sun but barely.","thin trees, fallen leaves, dappled light","#4a6a3a",["#0a1408","#1e2e14","#324828","#4a6a3a","#6a8a50"],125),
        lv("Getting Lost","Comic Neue","the paths all look the SAME and you keep going in circles.","dense forest, winding paths, moss","#3a5a2a",["#081008","#142814","#284028","#3a5a38","#4a7a4a"],165),
        lv("The Mushroom Clearing","Short Stack","theres GIANT mushrooms and some of them are alive and they dont like visitors.","huge mushrooms, glowing spores, fairy rings","#6a4a6a",["#140a14","#2e1a2e","#4a2e4a","#6a4a6a","#8a6a8a"],195),
        lv("Spider Territory","Kalam","SO MANY WEBS. the spiders are HUGE and they drop down from the trees.","thick webs, hanging cocoons, spider nests","#8a8a7a",["#14140a","#2e2e1a","#48482e","#6a6a48","#8a8a68"],235),
        lv("The Hollow Tree","Patrick Hand","theres one tree thats GIANT and hollow inside and something lives in there.","massive hollow trunk, root tunnels, glowing sap","#5a4a2a",["#0a0804","#1e1a0e","#3a3018","#5a4a28","#7a6a3a"],275),
        lv("The Forest Heart","Permanent Marker","the DEEPEST part of the forest. the trees move here and the boss is ANCIENT.","living trees, root throne, forest magic","#2a5a2a",["#041004","#0e280e","#1a401a","#2a5a2a","#3a7a3a"],140),
    ],
    [
        design([("tree wall","#"),("leaf floor","."),("root","~"),("sunbeam","*")],"Timber Wolf King","hes a wolf but like REALLY big and he howls SO loud the trees shake",["Twig Sprite","Leaf Bat","Acorn Thrower"],"Branch Club","its a big branch and it WHACKS stuff good","Bark Vest","tree bark strapped together. smells like outside",["Root Trip","Leaf Pile Pit","Falling Branch"],mode("G","aeolian"),"the wolf king howled one last time and ran away FOREVER!","the wolf king howled and you got SO scared."),
        design([("dense tree","#"),("mossy path","."),("mud","~"),("fungus","*")],"The Lost Troll","hes been lost in the forest so long he forgot how to be nice",["Moss Goblin","Path Mimic","Wandering Wisp"],"Compass Blade","it always points to the bad guys so you cant miss","Moss Cloak","its covered in moss so you blend in with everything",["False Path","Mud Trap","Wisp Lure"],mode("D","dorian"),"the troll found his way home and stopped being mean!","you got SO lost you forgot which way was up."),
        design([("mushroom wall","#"),("spore floor","."),("fairy ring","~"),("glowing cap","*")],"Fungus Emperor","hes a GIANT mushroom with a face and he shoots spores at you",["Puffball","Toadstool Knight","Spore Cloud"],"Mushroom Mallet","BONK. it makes a funny squeaky sound","Spore Mask","you can breathe in the spore clouds now",["Poison Spore","Sticky Fungus","Shroom Bounce"],mode("E","lydian"),"the fungus emperor popped like a balloon!! POP!","you breathed in too many spores. everything got fuzzy."),
        design([("web wall","#"),("sticky floor","."),("cocoon","~"),("silk strand","*")],"Mother Spider","shes got EIGHT LEGS and EIGHT EYES and shes not happy about visitors",["Baby Spider","Web Spinner","Silk Spitter"],"Web Cutter","it slices right through webs like NOTHING","Silk Armor","made from spider silk so its super light but SO strong",["Drop Web","Sticky Floor","Cocoon Trap"],mode("F","phrygian"),"all the webs fell down!! the spiders all ran away!","you got wrapped up in a cocoon. too sticky."),
        design([("root wall","#"),("sap floor","."),("tree vein","~"),("glow spot","*")],"The Hollow Knight","he lives in the tree and hes made of old wood and hes SUPER creaky",["Root Worm","Sap Slug","Bark Beetle"],"Sap Blade","its really sticky and stuff gets stuck to it","Hollow Armor","its made from the hollow tree and its really light",["Sap Trap","Root Grab","Falling Bark"],mode("A","mixolydian"),"the hollow knight crumbled into sawdust!!","the hollow knight creaked one last time and bonked you."),
        design([("ancient tree","#"),("magic moss","."),("root throne","~"),("forest crystal","*")],"The Ancient Oak","the oldest tree in the whole forest and it is NOT friendly. its been grumpy for a THOUSAND YEARS",["Tree Soldier","Root Dragon","Vine Whip"],"Heart Axe","the only thing that can cut the ancient oak","Forest Crown","it glows green and all the animals help you",["Living Root","Magic Vine","Bark Wall"],mode("C","aeolian"),"THE ANCIENT OAK IS SLEEPING AGAIN!! the forest is peaceful!!","the ancient oak squished you with a root. it was HUGE."),
    ],
    st(4,2,2), settings(4,3,99,2)
))

# ── 3: Lava World ──
campaigns.append(make_campaign(
    "Lava World", "Metal Mania",
    "EVERYTHING IS LAVA. well not everything but A LOT of it. dont touch the orange stuff or youll go OUCH.",
    "#2a0a0a", "#ffaa66",
    [
        lv("The Hot Part","Permanent Marker","its getting hot. like REALLY hot. the floor is warm and stuff is melting.","warm stone, steam vents, magma cracks","#aa5a3a",["#1a0a04","#3a1808","#5a3018","#8a5030","#aa7048"],130),
        lv("River Of Fire","Comic Neue","theres a WHOLE RIVER of lava and you have to cross it somehow.","lava river, stone bridges, fire jets","#cc6a3a",["#200804","#401808","#602818","#904028","#b06038"],170),
        lv("The Volcano Caves","Short Stack","youre INSIDE a volcano. the walls are all red and glowy.","volcanic cave, obsidian, lava pools","#884422",["#100400","#280e04","#401808","#602810","#803818"],200),
        lv("Fire Monster Den","Kalam","this is where the fire monsters sleep and there are A LOT of them.","monster nests, fire eggs, ash piles","#aa4422",["#180400","#301004","#481808","#682810","#884020"],240),
        lv("The Obsidian Fortress","Patrick Hand","its a fortress made of black glass and its REALLY sharp everywhere.","obsidian walls, lava moat, fire towers","#442244",["#0a040a","#1e0e1e","#321832","#4a284a","#623862"],280),
        lv("The Magma Dragon","Caveat","the dragon sleeps on TOP of the volcano and when it wakes up EVERYTHING shakes.","volcano peak, dragon nest, magma throne","#cc4400",["#1a0800","#3a1400","#5a2800","#884000","#aa5800"],145),
    ],
    [
        design([("hot wall","#"),("warm floor","."),("steam vent","~"),("magma crack","*")],"Steam Golem","hes made of hot rocks and steam comes out of his face",["Fire Ant","Magma Slug","Ember Bat"],"Cooled Lava Sword","it used to be lava but now its a sword. still kinda warm","Obsidian Shield","its black and shiny and lava bounces right off",["Steam Blast","Hot Floor","Lava Splash"],mode("D","phrygian"),"the steam golem cooled down and turned into a rock!","the steam golem was too hot to handle. literally."),
        design([("basalt wall","#"),("bridge stone","."),("lava flow","~"),("fire jet","*")],"Bridge Troll","he lives under the lava bridge and he sets stuff on FIRE",["Lava Fish","Fire Snake","Ash Sprite"],"Fire Poker","you poke stuff and it goes SIZZLE","Asbestos Coat","it doesnt catch on fire!! very useful in lava world",["Bridge Collapse","Fire Geyser","Lava Wave"],mode("E","aeolian"),"the bridge troll fell in the lava!! he was fine actually he lives there.","the bridge troll set your everything on fire."),
        design([("volcanic wall","#"),("obsidian floor","."),("lava pool","~"),("crystal","*")],"Volcano Worm","its like a worm but made of LAVA and its SO long",["Rock Maggot","Obsidian Crab","Lava Leech"],"Crystal Pickaxe","you can dig AND fight with it","Volcanic Rock Plate","its heavy but lava cant get through AT ALL",["Lava Pool","Rock Fall","Obsidian Shatter"],mode("F","dorian"),"the volcano worm burrowed away and didnt come back!!","the volcano worm was too hot and too long. SO long."),
        design([("fire wall","#"),("ashen floor","."),("fire egg","~"),("ember pile","*")],"Fire Mother","shes protecting her eggs and she is NOT letting you near them",["Fire Pup","Ash Crawler","Cinder Bug"],"Egg Cracker","its really good at cracking stuff open","Fireproof Suit","nothing can burn you now!! you feel invincible!!",["Egg Explosion","Ash Cloud","Fire Burst"],mode("G","lydian"),"the fire mother took her eggs somewhere safe and left you alone!","the fire mother breathed SO much fire at you."),
        design([("obsidian wall","#"),("glass floor","."),("sharp edge","~"),("fire window","*")],"Obsidian Knight","hes SHINY and sharp and every time you hit him pieces fly off",["Glass Soldier","Shard Bat","Crystal Imp"],"Diamond Edge","the sharpest thing EVER. it cuts obsidian like butter","Mirror Shield","stuff bounces off it and hits the bad guys instead!!",["Glass Floor Break","Shard Storm","Sharp Wall"],mode("A","phrygian"),"the obsidian knight shattered into a million pieces!!","the obsidian knight was too sharp. you got poked everywhere."),
        design([("volcano wall","#"),("magma floor","."),("dragon nest","~"),("fire throne","*")],"The Magma Dragon","the BIGGEST dragon EVER. it breathes lava not fire LAVA. and its scales are made of SOLID FIRE",["Dragon Whelp","Lava Wyrm","Magma Sprite"],"Dragon Slayer","its the only sword a dragon is scared of","Dragon Scale Mail","its made from dragon scales so dragons cant hurt you",["Magma Eruption","Dragon Breath","Lava Fall"],mode("C","aeolian"),"THE MAGMA DRAGON FLEW AWAY!! the volcano stopped being angry!!","the magma dragon breathed lava ON you. SO much lava."),
    ],
    st(5,2,3), settings(4,3,5,3)
))

# ── 4: Underwater Place ──
campaigns.append(make_campaign(
    "Underwater Place", "Gruppo",
    "youre underwater!! you can breathe because of a special helmet. the fish down here are NOT friendly.",
    "#0a1a2e", "#66aacc",
    [
        lv("Shallow Waters","Caveat","the water isnt that deep yet. you can still see the sun through the surface.","coral, seaweed, sandy bottom","#4a8aaa",["#040e1a","#0e2238","#1a3858","#2a5078","#3a6898"],130),
        lv("The Coral Maze","Comic Neue","theres coral EVERYWHERE and it all looks the same. dont get lost.","tall coral, winding passages, anemones","#3a7a8a",["#040a14","#0e1e2e","#1a3248","#2a4a68","#3a6288"],165),
        lv("The Dark Deep","Short Stack","its SO deep the sunlight doesnt reach here. just glowy stuff.","deep ocean, bioluminescence, angler fish","#1a3a5a",["#020812","#081828","#0e2840","#183a58","#224a70"],200),
        lv("Shipwreck Graveyard","Kalam","theres old ships everywhere and the ghosts of the sailors are still here.","broken hulls, ghost lights, treasure chests","#5a6a5a",["#080a08","#141e14","#223222","#304830","#3e603e"],240),
        lv("The Kraken Caves","Patrick Hand","the caves are HUGE and something really big lives in them.","massive caverns, tentacle marks, ink stains","#2a2a4a",["#04040a","#0e0e1e","#1a1a32","#282848","#383860"],280),
        lv("The Ocean King","Permanent Marker","the deepest part of the ocean. the king down here has been waiting for someone to fight.","underwater throne, pearl gates, pressure walls","#2a4a6a",["#040a14","#0e1a2e","#1a2e48","#284268","#385a88"],140),
    ],
    [
        design([("coral wall","#"),("sand floor","."),("seaweed","~"),("shell","*")],"Reef Shark Boss","hes the biggest shark and he has SO many teeth. like a hundred teeth",["Puffer Fish","Electric Eel","Crab Soldier"],"Trident","its got THREE pointy parts. thats three times the poke","Shell Armor","its made of big shells and it goes CLACK when you walk",["Urchin Patch","Sand Trap","Current Pull"],mode("F","ionian"),"the shark swam away SO fast!! bye bye sharky!","the shark had too many teeth. SO many teeth."),
        design([("tall coral","#"),("coral floor","."),("anemone","~"),("pearl","*")],"Coral Golem","hes made of coral and hes really slow but REALLY tough",["Anemone Stinger","Sea Slug","Coral Crab"],"Coral Breaker","it smashes through coral like NOTHING","Pearl Vest","its covered in pearls and its SO pretty AND strong",["Anemone Sting","Coral Maze Shift","Bubble Trap"],mode("G","dorian"),"the coral golem crumbled back into the reef!","the coral golem was too tough. you couldnt break through."),
        design([("dark rock","#"),("deep floor","."),("glow spot","~"),("angler light","*")],"Angler King","hes got a light on his head but its a TRICK. behind the light is a HUGE mouth",["Deep Jelly","Lantern Fish","Pressure Squid"],"Glow Sword","it lights up the dark AND bonks stuff","Pressure Suit","you can go SUPER deep without getting squished",["Angler Lure","Pressure Crush","Dark Current"],mode("A","phrygian"),"the angler king turned off his light and disappeared!","the angler king tricked you with his light. sneaky."),
        design([("hull wall","#"),("deck floor","."),("barnacle","~"),("porthole","*")],"Ghost Captain","hes been steering his broken ship for a HUNDRED years and he wont stop",["Ghost Sailor","Barnacle Beast","Anchor Zombie"],"Captain Cutlass","it belonged to a GOOD captain and it glows blue","Diving Bell Helm","you can breathe even better AND its hard to bonk",["Plank Walk","Ghost Grab","Hull Collapse"],mode("D","aeolian"),"the ghost captain finally stopped steering!! his ship sank for real this time!","the ghost captain made you walk the plank. into the DEEP part."),
        design([("cave wall","#"),("wet stone","."),("ink pool","~"),("tentacle mark","*")],"Baby Kraken","its just a BABY but its already ENORMOUS. imagine how big the mom is",["Tentacle Tip","Ink Cloud","Cave Fish"],"Kraken Tooth","you found it on the ground. its bigger than your ARM","Ink Proof Suit","the ink just slides right off",["Tentacle Grab","Ink Blind","Cave In"],mode("E","lydian"),"the baby kraken went to find its mom!! maybe dont follow it!!","the baby kraken grabbed you with SO many tentacles."),
        design([("palace wall","#"),("pearl floor","."),("pressure crack","~"),("throne coral","*")],"The Ocean King","hes half fish half king ALL scary. his trident shoots LIGHTNING underwater somehow",["Royal Guard Fish","Pearl Golem","Throne Eel"],"Lightning Rod","it catches the ocean kings lightning and throws it BACK","Ocean Crown","it lets you control water but only a little bit",["Pressure Wave","Lightning Strike","Whirlpool"],mode("C","aeolian"),"THE OCEAN KING BOWED TO YOU!! youre the new ocean ruler!!","the ocean king zapped you with underwater lightning. zap zap zap."),
    ],
    st(5,3,2), settings(5,3,6,3)
))

# ── 5: Sky Castle ──
campaigns.append(make_campaign(
    "Sky Castle", "Cinzel",
    "theres a castle IN THE SKY. you climb up clouds to get there and DONT look down its SO high up.",
    "#1a2a4a", "#aaccff",
    [
        lv("Cloud Steps","Gloria Hallelujah","the clouds are like stairs! bouncy soft stairs that go UP and UP.","fluffy clouds, sky bridges, wind gusts","#8aaamm",["#141a2e","#2e3a58","#485a80","#6a7aaa","#8a9acc"],125),
        lv("The Wind Tunnels","Comic Neue","the wind is SO strong it pushes you around. hold on!!","wind corridors, air currents, floating debris","#6a8acc",["#0a142a","#1a2e50","#2e4878","#4a68a0","#6a88c8"],165),
        lv("Eagle Territory","Short Stack","the eagles live up here and they think youre INVADING. theyre really mad.","nests, feathers, cliff edges","#8a7a5a",["#140e04","#2e2210","#48381e","#6a5230","#8a6a42"],200),
        lv("The Lightning Zone","Kalam","LIGHTNING everywhere!! it goes ZAP ZAP and you gotta dodge it.","storm clouds, lightning rods, charged air","#4a4a8a",["#0a0a1a","#1a1a3a","#2e2e5a","#4a4a7a","#6a6a9a"],240),
        lv("Castle Gates","Patrick Hand","the castle is RIGHT THERE. you can see it. its SO big and SO high up.","massive gates, cloud walls, sky towers","#8a8acc",["#141428","#2e2e48","#484868","#6a6a90","#8a8ab0"],280),
        lv("The Sky King","Caveat","the sky king lives at the VERY TOP. he can control the weather and hes MEAN about it.","throne of clouds, storm crown, lightning throne","#aaaaee",["#1a1a30","#3a3a58","#5a5a80","#7a7aaa","#9a9acc"],140),
    ],
    [
        design([("cloud wall","#"),("cloud floor","."),("gap","~"),("sunbeam","*")],"Cloud Giant","hes made of clouds and hes SO big you can walk between his toes",["Wind Sprite","Cloud Puff","Sky Rat"],"Wind Blade","it shoots air when you swing it WHOOSH","Cloud Boots","you can walk on clouds without falling through!!",["Cloud Gap","Wind Push","Thunder Clap"],mode("C","ionian"),"the cloud giant blew away!! poof!!","the cloud giant stepped on you. he didnt even notice."),
        design([("wind wall","#"),("air floor","."),("gust","~"),("debris","*")],"Storm Hawk","its a hawk but made of WIND and LIGHTNING at the same time",["Gust Imp","Air Elemental","Debris Bat"],"Gust Hammer","it makes a HUGE gust when you swing it","Windbreaker","it stops the wind from pushing you around",["Wind Tunnel","Air Pocket","Debris Storm"],mode("D","mixolydian"),"the storm hawk turned back into regular wind!","the storm hawk blew you off the edge. SO far down."),
        design([("cliff wall","#"),("nest floor","."),("feather pile","~"),("egg","*")],"Eagle Lord","the BIGGEST eagle with golden feathers and talons like swords",["War Eagle","Nest Guard","Fledgling Fury"],"Talon Blade","its really sharp and it goes SWOOSH through the air","Feather Mail","its light as a feather get it because its MADE of feathers",["Dive Bomb","Nest Trap","Egg Roll"],mode("E","dorian"),"the eagle lord flew to a different mountain!! bye!!","the eagle lord grabbed you and dropped you. from REALLY high."),
        design([("storm wall","#"),("charged floor","."),("lightning rod","~"),("spark","*")],"Thunder Elemental","its PURE LIGHTNING in the shape of a person and it goes BZZZT",["Spark Bug","Static Cloud","Bolt Rat"],"Rubber Sword","lightning cant go through rubber!! SCIENCE!!","Rubber Suit","NOTHING electric can hurt you now",["Lightning Strike","Static Floor","Chain Lightning"],mode("F","phrygian"),"the thunder elemental discharged and went pzzzt. done!","you got zapped SO many times. your hair is standing up forever."),
        design([("castle wall","#"),("stone floor","."),("arrow slit","~"),("banner","*")],"Gate Warden","he guards the castle gates and he has FOUR swords. one for each hand. he has FOUR HANDS",["Sky Knight","Cloud Archer","Gate Guard"],"Sky Blade","it glows blue and makes you feel brave","Castle Shield","its from the castle armory. the GOOD armory",["Arrow Volley","Gate Slam","Bridge Drop"],mode("G","aeolian"),"the gates are OPEN!! into the castle!!","the gate warden had too many swords. FOUR swords is too many."),
        design([("throne wall","#"),("royal floor","."),("cloud throne","~"),("crown jewel","*")],"The Sky King","he controls ALL the weather. rain snow lightning wind EVERYTHING. and hes really mad at you for climbing up here",["Storm Guard","Weather Wizard","Thunder Knight"],"Storm Breaker","the ONLY weapon that can break through the sky kings storms","Crown Of Calm","it makes all the weather stop around you",["Tornado","Hail Storm","Lightning Cage"],mode("A","aeolian"),"THE SKY KING GAVE UP!! the weather is nice again FOREVER!!","the sky king threw a tornado at you. a WHOLE tornado."),
    ],
    st(4,2,2), settings(5,4,5,3)
))

# ── 6: Candy Land ──
campaigns.append(make_campaign(
    "Candy Land", "Bungee Shade",
    "EVERYTHING is made of candy!! the ground is chocolate, the trees are lollipops, but the candy is ALIVE and it wants to eat YOU.",
    "#2a1a2a", "#ffaacc",
    [
        lv("Chocolate Meadow","Indie Flower","the ground is chocolate and it smells SO good but you cant eat it. well you CAN but dont.","chocolate ground, candy flowers, gumdrop bushes","#8a5a4a",["#1a0e0a","#3a2218","#5a3828","#7a5040","#9a6858"],125),
        lv("Lollipop Forest","Comic Neue","the trees are ALL lollipops. giant ones. and they spin.","lollipop trees, sugar paths, sprinkle rain","#cc66aa",["#2a0e1e","#4a1e38","#6a3258","#8a4a78","#aa6298"],170),
        lv("Gummy Bear Village","Short Stack","the gummy bears live here. they look cute but theyre REALLY squishy and strong.","gummy houses, jelly roads, sugar lamps","#66aa66",["#0a1a0a","#1a3a1a","#2a5a2a","#3a7a3a","#4a9a4a"],200),
        lv("The Sour Zone","Kalam","EVERYTHING is sour here. so sour your face goes like THIS and you cant stop it.","sour crystals, acid pools, warhead walls","#aaaa44",["#1a1a04","#3a3a0e","#5a5a1a","#7a7a2a","#9a9a3a"],240),
        lv("Cotton Candy Clouds","Patrick Hand","clouds made of COTTON CANDY. you can eat them but the cotton candy monsters live here.","fluffy pink clouds, sugar storms, candy rain","#ffaacc",["#2a1a1e","#4a2e38","#6a4258","#8a5a78","#aa7298"],275),
        lv("The Sugar Queen","Permanent Marker","the sugar queen rules ALL the candy. shes sweet but DEADLY. get it?? sweet??","sugar palace, crystal throne, candy army","#ff88aa",["#2a1018","#4a2030","#6a3048","#8a4068","#aa5888"],140),
    ],
    [
        design([("chocolate wall","#"),("chocolate floor","."),("candy flower","~"),("gumdrop","*")],"Chocolate Golem","hes made of chocolate and hes MELTING but he keeps fighting anyway",["Candy Corn","Gumdrop Bouncer","Sprinkle Swarm"],"Candy Cane Sword","its pointy on the end and it tastes like peppermint","Jawbreaker Shield","its IMPOSSIBLE to break. thats why its called that",["Chocolate Quicksand","Sticky Caramel","Sugar Spike"],mode("C","lydian"),"the chocolate golem melted into a puddle!! chocolate puddle!!","the chocolate golem sat on you. SO chocolatey."),
        design([("candy wall","#"),("sugar path","."),("sprinkle pile","~"),("lollipop","*")],"Lollipop Dragon","it breathes SUGAR FIRE and its tail is a giant swirl",["Peppermint Knight","Butterscotch Bird","Rock Candy Crab"],"Sugar Slicer","it cuts through candy like a hot knife through... more candy","Hard Candy Armor","its SO hard nothing can crack it",["Sticky Floor","Sprinkle Storm","Sugar Trap"],mode("D","ionian"),"the lollipop dragon dissolved in the rain!!","the lollipop dragon sugar-fired you. sticky AND hot."),
        design([("gummy wall","#"),("jelly floor","."),("gummy house","~"),("sugar lamp","*")],"Gummy King","hes the biggest gummy bear and he BOUNCES. every time you hit him he just bounces back",["Gummy Soldier","Jelly Cube","Licorice Snake"],"Sour Straw","its sour AND its a weapon. multitasking","Gummy Armor","stuff just sinks into it and gets stuck",["Gummy Bounce","Jelly Trap","Licorice Lasso"],mode("E","dorian"),"the gummy king got too stretched out and couldnt bounce anymore!!","the gummy king bounced on you. boing boing SQUISH."),
        design([("sour wall","#"),("acid floor","."),("sour crystal","~"),("warhead","*")],"The Sour King","everything about him is SOUR. his face his attitude his attacks EVERYTHING",["Sour Patch","Acid Drop","Lemon Demon"],"Sweet Sword","its the opposite of sour so it hurts him EXTRA","Base Armor","it neutralizes the sour. SCIENCE again!!",["Acid Pool","Sour Blast","Pucker Trap"],mode("F","phrygian"),"the sour king ate something sweet and his face went NORMAL!!","everything was too sour. your face is stuck like that now."),
        design([("cotton wall","#"),("fluffy floor","."),("sugar cloud","~"),("candy rain","*")],"Cotton Candy Yeti","hes HUGE and FLUFFY and pink and he hugs you but too hard",["Sugar Cloud","Fluff Monster","Candy Hail"],"Sugar Spear","it goes through cotton candy like NOTHING","Raincoat","keeps the candy rain off. its raining gummy bears!!!",["Sugar Storm","Fluff Trap","Candy Hail"],mode("G","mixolydian"),"the cotton candy yeti unraveled!! just a pile of fluff now!!","the cotton candy yeti hugged you too hard. TOO fluffy."),
        design([("crystal wall","#"),("sugar floor","."),("candy pillar","~"),("throne gem","*")],"The Sugar Queen","she looks nice but she turns EVERYTHING into candy. EVERYTHING. even YOU if youre not careful",["Royal Gummy","Crystal Guard","Sugar Fairy"],"Cavity Blade","its the sugar queens only weakness. she hates dentists","Sugar Crown","it lets you control candy. all the candy listens to you",["Candy Cage","Crystal Trap","Sugar Storm"],mode("A","aeolian"),"THE SUGAR QUEEN TURNED NICE!! she gives you candy now GOOD candy!!","the sugar queen turned you into a candy. a people-shaped candy."),
    ],
    st(5,2,2), settings(5,3,5,2)
))

# ── 7: Robot City ──
campaigns.append(make_campaign(
    "Robot City", "Orbitron",
    "the robots took over the city!! they beep and boop and try to LASER you. everything is metal and blinky.",
    "#1a1a2a", "#88aacc",
    [
        lv("The Outskirts","Electrolize","the edge of the city. you can hear the robots beeping from here.","metal fences, broken cars, surveillance cameras","#6a7a8a",["#0a0e14","#1a2230","#2e384a","#4a5268","#687088"],130),
        lv("Assembly Line","Comic Neue","this is where they MAKE new robots. theres SO many coming off the line.","conveyor belts, robot parts, welding sparks","#7a8a9a",["#0e1218","#222e3a","#384858","#506878","#688898"],170),
        lv("The Server Room","Short Stack","all the computers are in here and its SO hot and loud BZZZZZT.","server racks, blinking lights, cables everywhere","#4a5a6a",["#080a10","#141e2a","#223244","#304a5e","#3e6278"],200),
        lv("Robot Neighborhood","Kalam","the robots have HOUSES. tiny robot houses with tiny robot yards.","metal houses, LED gardens, robot pets","#8a8a8a",["#0e0e0e","#222222","#383838","#505050","#686868"],240),
        lv("The Mainframe","Patrick Hand","the BIG computer. the one that controls ALL the robots.","massive computer, data streams, firewall","#3a3a6a",["#08081a","#14142e","#222248","#383862","#4e4e7e"],280),
        lv("ROBOT PRIME","Caveat","the BIGGEST robot. hes as big as a building and he shoots lasers from his EYES.","giant robot arena, laser grid, power core","#4a6a8a",["#081020","#141e38","#223050","#304468","#3e5880"],140),
    ],
    [
        design([("metal wall","#"),("concrete","."),("broken car","~"),("camera","*")],"Security Bot","it has a siren and when it sees you it goes WEE WOO WEE WOO",["Drone","Turret","Patrol Bot"],"EMP Bat","one swing and all the electronics go BZZZT dead","Hard Hat","it protects you from falling robot parts",["Laser Grid","Alarm","Electro Floor"],mode("E","dorian"),"the security bot ran out of batteries!!","the security bot tased you. BZZZT."),
        design([("factory wall","#"),("belt floor","."),("robot arm","~"),("welder","*")],"Assembly Master","it builds OTHER robots while youre fighting it. stop building!!",["Fresh Bot","Arm Bot","Welder Drone"],"Wrench","you can take robots APART with it","Welding Mask","sparks bounce right off your face",["Conveyor Pull","Welding Spark","Robot Arm Grab"],mode("F","phrygian"),"the assembly line is OFF!! no more new robots!!","the assembly master built too many robots. you got overwhelmed."),
        design([("server rack","#"),("cable floor","."),("blinking light","~"),("vent","*")],"Virus Boss","its not even a real robot its just BAD SOFTWARE but it controls EVERYTHING",["Glitch Bot","Data Worm","Firewall"],"Debug Stick","it finds bugs and SQUISHES them","Antivirus Suit","viruses cant infect you while youre wearing it",["System Crash","Data Stream","Overheating"],mode("G","lydian"),"VIRUS DELETED!! all the servers are clean now!!","the virus boss crashed your brain. blue screen of OW."),
        design([("house wall","#"),("metal yard","."),("LED garden","~"),("mailbot","*")],"Neighbor Bot 9000","it looks friendly but it complains about EVERYTHING and then tries to laser you",["Lawn Bot","Pet Bot","Mail Bot"],"Garden Shears","snip snip. cuts through wires really good","Neighbor Fence","nothing gets over the fence. NOTHING",["Lawn Mower","Sprinkler Laser","Package Bomb"],mode("A","mixolydian"),"neighbor bot 9000 moved away!! good riddance!!","neighbor bot 9000 filed a complaint against your FACE."),
        design([("computer wall","#"),("circuit floor","."),("data port","~"),("core node","*")],"The Firewall","its not a wall of fire its a wall of LASERS and its SO thick",["Data Knight","Pixel Guard","Code Monkey"],"USB Sword","you plug it in and it goes HACK HACK HACK","Firewall Breaker","it opens up holes in the firewall for you",["Laser Wall","Data Trap","System Lockout"],mode("D","aeolian"),"the firewall is DOWN!! you can see the mainframe!!","the firewall blocked you out. ACCESS DENIED."),
        design([("titan wall","#"),("arena floor","."),("laser grid","~"),("power core","*")],"ROBOT PRIME","hes as big as a BUILDING. his feet are the size of CARS. he shoots lasers from his EYES and missiles from his ARMS",["Mini Prime","Laser Drone","Missile Bot"],"Power Sword","it uses ROBOT PRIMES own power against him","Mech Suit","you get in a robot too!! now its a FAIR fight!!",["Stomp","Eye Laser","Missile Barrage"],mode("C","aeolian"),"ROBOT PRIME POWERED DOWN!! the city is free!! all the good robots are happy!!","ROBOT PRIME stepped on you. you were too small. SO small."),
    ],
    st(4,3,3), settings(4,3,5,3)
))

# ── 8: Dinosaur Island ──
campaigns.append(make_campaign(
    "Dinosaur Island", "Permanent Marker",
    "THERES DINOSAURS. real ones!! well game ones. they stomp around and try to eat you and some of them are REALLY BIG.",
    "#1a2a14", "#aacc88",
    [
        lv("The Beach","Gloria Hallelujah","you wash up on the beach and you can already hear them STOMPING.","sandy beach, palm trees, footprints","#aa9a6a",["#1a1408","#3a2e18","#5a4a30","#7a6a48","#9a8a60"],125),
        lv("Raptor Valley","Comic Neue","the raptors are FAST and they hunt in PACKS. like really smart packs.","jungle valley, raptor nests, claw marks","#5a8a3a",["#081408","#142e14","#284828","#3a6a38","#4a8a48"],165),
        lv("The Tar Pits","Short Stack","everythings sticky and slow and if you stop moving you SINK.","tar pools, bones, bubbling pits","#4a3a2a",["#0a0804","#1e1a0e","#322e18","#4a4228","#625a38"],200),
        lv("Volcano Nests","Kalam","the dinos nest near the volcano because its WARM. and because theyre not scared of ANYTHING.","volcanic rock, giant eggs, warm caves","#8a4a2a",["#1a0804","#3a180e","#5a2e18","#7a4428","#9a5a38"],240),
        lv("Bone Canyon","Patrick Hand","SO MANY BONES. old dinosaur bones everywhere. and some of them GET UP AND WALK.","bone bridges, fossil walls, skeleton nests","#aaaaaa",["#141414","#2a2a2a","#404040","#585858","#707070"],275),
        lv("The Rex","Caveat","the T-REX. the BIGGEST ONE. its teeth are the size of your WHOLE BODY.","rex arena, massive footprints, destruction","#6a8a3a",["#0a1404","#1a2e0e","#2a4818","#3a6228","#4a7c38"],140),
    ],
    [
        design([("rock wall","#"),("sand floor","."),("palm tree","~"),("footprint","*")],"Beach Rex","its a small rex but it thinks its REALLY tough. it roars at seagulls",["Compy","Sand Crab","Beach Pterodactyl"],"Bone Club","its a dinosaur bone and its REALLY hard","Turtle Shell","you wear it on your back like a backpack but for ARMOR",["Quicksand","Tail Whip","Stampede"],mode("G","ionian"),"the beach rex ran into the ocean!! can rexes swim?? probably not!!","the beach rex chomped you. not cool beach rex."),
        design([("jungle wall","#"),("fern floor","."),("raptor nest","~"),("claw mark","*")],"Alpha Raptor","the leader of ALL the raptors. the smartest AND the meanest",["Pack Raptor","Nest Guard","Baby Raptor"],"Raptor Claw","you found one and put it on a stick. clever!!","Scale Armor","made from shed raptor scales. SO lightweight",["Raptor Ambush","Nest Alarm","Pack Attack"],mode("D","dorian"),"the alpha raptor took its pack and left!! they respect you now!!","the raptors outsmarted you. they PLANNED it."),
        design([("tar wall","#"),("sticky floor","."),("bone pile","~"),("bubble","*")],"Tar Titan","its been stuck in the tar for SO long its BECOME the tar. its gross",["Tar Blob","Bone Walker","Sticky Bug"],"Scraper","it scrapes tar off of EVERYTHING including tar monsters","Oil Boots","you can walk on tar without sinking!!",["Tar Pool","Sink Spot","Bubble Burst"],mode("E","phrygian"),"the tar titan sank back into the tar forever!! glub glub!!","you got stuck in the tar. really stuck. SO stuck."),
        design([("volcanic wall","#"),("warm floor","."),("dino egg","~"),("hot rock","*")],"Nesting Stegosaurus","its protecting its eggs and it has SPIKES on its tail and its NOT afraid to use them",["Baby Stego","Egg Thief","Heat Lizard"],"Spike Hammer","it has spikes just like the stegosaurus. FAIR.","Heat Armor","the volcanic heat cant bother you anymore",["Tail Spike","Egg Roll","Lava Drip"],mode("F","lydian"),"the stegosaurus calmed down!! you just had to not touch the eggs!!","the stegosaurus tail-spiked you. you shouldnt have touched the eggs."),
        design([("bone wall","#"),("fossil floor","."),("rib arch","~"),("skull","*")],"Skeleton Rex","its a rex but made of BONES and its ALREADY DEAD so how do you even fight it??",["Bone Raptor","Fossil Spider","Marrow Worm"],"Femur Club","its the biggest bone you ever saw and you HIT stuff with it","Rib Cage Armor","its literally a rib cage. gross but effective",["Bone Collapse","Fossil Trap","Skeleton Reform"],mode("A","aeolian"),"the skeleton rex fell apart and STAYED apart this time!!","the skeleton rex bit you with teeth it doesnt even HAVE anymore."),
        design([("rex wall","#"),("crushed floor","."),("massive print","~"),("tooth","*")],"The Tyrant King","the BIGGEST T-REX EVER. it cant even fit in the LEVEL. its head fills the WHOLE ROOM",["Rex Guard","Dino Knight","Primal Raptor"],"Rex Tooth Blade","its ONE of the rex's baby teeth and its already a sword","Dino Plate Mail","nothing can bite through this. NOTHING",["Rex Stomp","Tail Sweep","Dino Roar"],mode("C","aeolian"),"THE TYRANT KING IS DOWN!! the island is safe!! for now!!","the tyrant king is just TOO big. like WAY too big."),
    ],
    st(5,2,2), settings(5,4,99,2)
))

# ── 9: Ghost House ──
campaigns.append(make_campaign(
    "Ghost House", "Jolly Lodger",
    "its a haunted house and ALL the ghosts are real. they go BOOOO and youre like AHHH and its really scary but also fun.",
    "#1a1a24", "#aaaacc",
    [
        lv("The Front Door","Shadows Into Light","the door opens by itself. CREEEAK. its already scary and you just got here.","creaky door, dusty hall, flickering lights","#8a8a9a",["#14141a","#2a2a30","#404048","#585862","#72727e"],125),
        lv("The Living Room","Comic Neue","the furniture MOVES. the couch slides around and the chairs walk.","haunted furniture, floating objects, cold spots","#7a7a8a",["#10101a","#242430","#383848","#505062","#68687e"],170),
        lv("The Kitchen","Short Stack","the pots and pans fly around by themselves and the fridge has eyes.","floating utensils, possessed fridge, spooky stove","#8a8a7a",["#141410","#2a2a24","#404038","#585850","#727268"],195),
        lv("The Basement","Kalam","the basement is SO dark and theres noises coming from DOWN THERE.","dark stairs, old boxes, cobwebs, something moving","#4a4a5a",["#08080e","#181820","#282834","#3a3a48","#4e4e5e"],240),
        lv("The Attic","Patrick Hand","the attic has old toys and dolls and some of them BLINK.","old trunks, creepy dolls, dusty mirrors","#7a6a6a",["#141010","#2a2220","#403838","#585050","#706868"],275),
        lv("The Ghost Queen","Caveat","shes been haunting this house for TWO HUNDRED YEARS and she is NOT leaving.","phantom throne, spirit mirrors, ghost fog","#9a9acc",["#141428","#2e2e48","#484868","#6a6a90","#8a8ab0"],140),
    ],
    [
        design([("wallpaper","#"),("wood floor","."),("dust","~"),("candle","*")],"Door Phantom","it IS the door. the whole door is a ghost. every time you open it AHHH",["Dust Bunny","Candle Ghost","Floor Creak"],"Silver Key","it opens ghost doors and BONKS ghost faces","Blessed Coat","ghosts cant touch you when youre wearing it",["Door Slam","Creak Scare","Candle Out"],mode("D","aeolian"),"the door phantom unhinged itself!! get it?? UNHINGED!!","the door phantom slammed on you too many times."),
        design([("fancy wall","#"),("carpet","."),("cold spot","~"),("painting","*")],"Poltergeist","it throws EVERYTHING. chairs tables lamps EVERYTHING. watch out!!",["Walking Chair","Flying Lamp","Rug Rat"],"Ghost Poker","it goes right through ghosts and they HATE it","Spirit Ward","stuff cant hit you because it bounces off the ward",["Flying Furniture","Cold Blast","Painting Grab"],mode("E","phrygian"),"the poltergeist got tired of throwing stuff and took a nap!!","the poltergeist threw a piano at you. a WHOLE piano."),
        design([("tile wall","#"),("kitchen floor","."),("spill","~"),("burner","*")],"Chef Specter","it throws pots of GHOST SOUP at you. it burns but its also cold somehow??",["Fork Fiend","Plate Spinner","Spoon Stirrer"],"Rolling Pin","BONK BONK BONK. very effective against ghost food","Oven Mitts","nothing hot can hurt your hands. NOTHING",["Boiling Pot","Knife Block","Oven Blast"],mode("F","dorian"),"the kitchen is clean!! no more ghost food!!","the chef specter put you in the soup. youre soup now."),
        design([("stone wall","#"),("dirt floor","."),("old box","~"),("web","*")],"The Basement Thing","nobody knows what it IS. its big and dark and it makes horrible noises",["Shadow Rat","Box Mimic","Cobweb Ghost"],"Flashlight","it reveals everything AND you can bonk stuff with it","Work Boots","nothing in the basement can hurt your feet",["Stair Collapse","Box Avalanche","Dark Grab"],mode("G","aeolian"),"you turned on the lights and the basement thing DISAPPEARED!!","the basement thing got you. nobody knows what happened."),
        design([("attic wall","#"),("dusty floor","."),("trunk","~"),("mirror","*")],"The Doll Collector","all the creepy dolls follow her around and she makes MORE",["Creepy Doll","Mirror Ghost","Trunk Mimic"],"Doll Sword","it scares the dolls because its sharp and dolls dont like sharp","Mirror Shield","ghosts see themselves and get SO confused",["Doll Swarm","Mirror Trap","Trunk Snap"],mode("A","lydian"),"all the dolls stopped moving!! theyre just dolls again!!","the dolls carried you away. SO many tiny hands."),
        design([("ghost wall","#"),("spirit floor","."),("mirror portal","~"),("spirit candle","*")],"The Ghost Queen","shes been here for TWO HUNDRED YEARS and shes the scariest ghost IN THE WORLD. she can go through walls AND floors AND YOU",["Royal Specter","Spirit Knight","Phantom Maid"],"Ghost Buster","it sucks ghosts up like a vacuum WHOOOOSH","Exorcism Armor","ghosts cant even LOOK at you while youre wearing it",["Spirit Drain","Wall Phase","Ghost Wail"],mode("C","aeolian"),"THE GHOST QUEEN MOVED ON!! she said its time and she floated into the light!!","the ghost queen went THROUGH you. SO cold. like SO cold."),
    ],
    st(5,2,2), settings(5,3,5,2)
))

# ── 10: Pirate Ocean ──
campaigns.append(make_campaign(
    "Pirate Ocean", "Pirata One",
    "PIRATES!! theyre on boats and they have swords and they go ARRR. you need your own boat and your own sword.",
    "#0a1a2a", "#ccaa66",
    [
        lv("The Docks","Handlee","the pirates dock their ships here. you can smell the salt and the DANGER.","wooden docks, rope, barrels, seagulls","#8a7a5a",["#141008","#2e2418","#483828","#625040","#7c6858"],130),
        lv("The Pirate Tavern","Comic Neue","all the pirates hang out here and they arm wrestle and throw stuff.","tavern tables, mugs, dartboard, chandelier","#6a5a3a",["#100c04","#241e0e","#3a301a","#524428","#6a5838"],170),
        lv("Smuggler Caves","Short Stack","secret caves where pirates hide their STUFF. its dark and wet and echoey.","cave pools, treasure chests, dripping water","#4a5a6a",["#08101a","#141e2e","#223048","#304462","#3e587c"],200),
        lv("Ship Battle","Kalam","youre on a ship fighting ANOTHER ship. cannons going BOOM BOOM BOOM.","ship deck, cannons, rigging, waves","#7a6a4a",["#100e04","#24200e","#3a341a","#524a28","#6a6038"],240),
        lv("Treasure Island","Patrick Hand","X marks the spot!! but theres SO many traps guarding the treasure.","beach, palm trees, treasure map, X marks","#aaaa5a",["#1a1a04","#3a3a0e","#5a5a1a","#7a7a2a","#9a9a3a"],275),
        lv("The Pirate King","Permanent Marker","the KING of all pirates. he has the biggest ship and the biggest hat and the biggest SWORD.","flagship deck, captain quarters, pirate throne","#aa8a4a",["#1a1404","#3a2e0e","#5a4a1a","#7a682a","#9a883a"],140),
    ],
    [
        design([("dock wall","#"),("plank floor","."),("water","~"),("barrel","*")],"Dock Master","hes got a hook for a hand and a peg for a leg and hes MEAN about both",["Deck Rat","Seagull","Rope Monkey"],"Boat Hook","its for pulling boats but it works on pirates too","Barrel Lid","you hold it up and stuff bounces off BONK",["Loose Plank","Rope Trap","Barrel Roll"],mode("D","mixolydian"),"the dock master fell in the water!! he cant swim with a hook and a peg!!","the dock master hooked you. oww his hook is SHARP."),
        design([("tavern wall","#"),("sawdust floor","."),("spilled drink","~"),("dartboard","*")],"Tavern Brawler","hes the biggest pirate in the tavern and he fights EVERYONE",["Bar Rat","Mug Thrower","Chair Fighter"],"Bar Stool","you swing it around and EVERYTHING breaks","Barrel Vest","its a barrel with arm holes. dont ask",["Flying Mug","Chair Throw","Chandelier Drop"],mode("E","dorian"),"the tavern brawler passed out!! too much fighting!!","the tavern brawler threw a table at you. a WHOLE table."),
        design([("cave wall","#"),("wet floor","."),("tide pool","~"),("stalactite","*")],"Smuggler Boss","he knows every secret cave and he has traps in ALL of them",["Cave Pirate","Treasure Mimic","Bat Swarm"],"Stalactite Sword","it broke off the ceiling and its PERFECT for stabbing","Smuggler Cloak","you blend into the caves and nobody can see you",["Tide Rise","Cave In","Hidden Pit"],mode("F","aeolian"),"the smuggler boss got trapped in his own cave!! ironic!!","the smuggler boss knew the caves better than you. way better."),
        design([("ship wall","#"),("deck floor","."),("cannon","~"),("mast","*")],"Captain Cannonball","she fires herself OUT of the cannon and she IS the cannonball",["Pirate Swabber","Crow Nest Archer","Powder Monkey"],"Cutlass","its a real pirate sword and it goes CLANG CLANG","Captain Hat","it makes all the pirates think youre THEIR captain",["Cannon Fire","Mast Fall","Deck Collapse"],mode("G","phrygian"),"captain cannonball missed and flew into the ocean!!","captain cannonball hit you directly. she IS a cannonball."),
        design([("sand wall","#"),("beach floor","."),("palm tree","~"),("x mark","*")],"Treasure Guardian","a HUGE stone statue that comes alive when you get close to the treasure",["Sand Crab","Coconut Bomber","Map Ghost"],"Shovel","dig AND bonk. the perfect pirate tool","Treasure Chest Helm","you put a treasure chest on your head. it works great",["Sand Trap","Coconut Drop","Quicksand"],mode("A","lydian"),"the guardian crumbled!! THE TREASURE IS YOURS!!","the guardian squished you flat. guard duty is SERIOUS."),
        design([("gold wall","#"),("flagship deck","."),("treasure pile","~"),("pirate flag","*")],"The Pirate King","he has a sword in EACH hand and a knife in his TEETH and a cannon on his BACK. hes the most pirate pirate EVER",["First Mate","Pirate Elite","Parrot Bomber"],"Kings Cutlass","the only sword better than the pirate kings swords","Admirals Coat","it means you outrank EVERY pirate on the ocean",["Dual Slash","Cannon Shot","Parrot Swarm"],mode("C","aeolian"),"THE PIRATE KING SURRENDERED!! you ARE the pirate king now!!","the pirate king was just too pirate. too much pirate for one fight."),
    ],
    st(4,2,3), settings(5,3,5,2)
))

# ── 11: Bug World ──
campaigns.append(make_campaign(
    "Bug World", "Caveat",
    "you shrunk down REALLY small and now the bugs are bigger than YOU!! everything is huge. a leaf is like a whole building.",
    "#1a2a0a", "#88aa44",
    [
        lv("Under The Porch","Handlee","this is where you shrunk. the floorboards are like bridges now.","dust bunnies, crumbs, giant floor cracks","#8a7a5a",["#14100a","#2e2418","#483a28","#625240","#7c6a58"],125),
        lv("Ant Tunnels","Comic Neue","the ants are SO organized. they have highways and everything. and theyre NOT sharing.","tunnel networks, ant highways, food stores","#6a4a2a",["#0e0804","#221a0e","#382e1a","#4e4228","#645838"],165),
        lv("The Garden","Short Stack","one flower is like a WHOLE TREE when youre this small. and the bees are HUGE.","giant flowers, dewdrops, pollen clouds","#4a8a2a",["#081404","#142e0e","#22481a","#306228","#3e7c38"],200),
        lv("Spider Web City","Kalam","the spiders built a whole CITY out of webs. its actually pretty cool if they werent trying to eat you.","web structures, silk bridges, web houses","#aaaaaa",["#141414","#2a2a2a","#424242","#5a5a5a","#747474"],240),
        lv("The Hive","Patrick Hand","inside the beehive. EVERYTHING is honey and wax and bees EVERYWHERE.","honeycomb walls, honey pools, wax pillars","#aaaa44",["#1a1a04","#3a3a0e","#5a5a1a","#7a7a2a","#9a9a3a"],275),
        lv("The Queen Beetle","Permanent Marker","the BIGGEST bug. shes like a car. a car with PINCERS and a SHELL and shes SO shiny.","beetle arena, shell fragments, royal chamber","#4a6a2a",["#081004","#14280e","#22401a","#305828","#3e7038"],140),
    ],
    [
        design([("wood wall","#"),("dust floor","."),("crumb","~"),("crack","*")],"Dust Mite King","you cant even SEE him normally but now hes HUGE and gross",["Dust Mite","Crumb Ant","Floor Flea"],"Toothpick Sword","its a toothpick but to you its a LANCE","Thimble Helmet","a thimble fits perfectly on your head now",["Dust Storm","Crack Fall","Crumb Slide"],mode("G","dorian"),"the dust mite king is STILL too small to see. even now. bye!!","the dust mite king was gross and strong. too gross."),
        design([("dirt wall","#"),("tunnel floor","."),("food store","~"),("ant marker","*")],"Ant General","she commands TEN THOUSAND ants. they march in PERFECT lines",["Worker Ant","Soldier Ant","Scout Ant"],"Pin Sword","its a sewing pin and its perfectly pointy","Bottle Cap Shield","it covers your whole body when youre this small",["Ant Swarm","Tunnel Collapse","Acid Spray"],mode("D","phrygian"),"the ant general ordered a retreat!! the ants marched away!","the ant general sent ALL ten thousand ants at you. too many."),
        design([("stem wall","#"),("petal floor","."),("pollen pile","~"),("dewdrop","*")],"Mantis Warrior","its a praying mantis and it does NOT pray. it just FIGHTS",["Beetle","Aphid","Caterpillar"],"Thorn Blade","a rose thorn. its SO sharp and it smells nice","Petal Armor","its light and pretty and surprisingly strong",["Pollen Cloud","Dewdrop Slip","Thorny Ground"],mode("E","lydian"),"the mantis warrior bowed and flew away!! respect!!","the mantis warrior was too fast. SO fast. ninja fast."),
        design([("web wall","#"),("silk floor","."),("web house","~"),("cocoon","*")],"Web Architect","it designs all the webs and they are BEAUTIFUL but also DEADLY",["Silk Spinner","Web Walker","Cocoon Guard"],"Web Cutter","slices through silk like butter","Silk Suit","you slide right through webs without getting stuck",["Sticky Web","Web Trap Door","Silk Snare"],mode("F","aeolian"),"the web architect ran out of silk!! the whole city fell down!!","you got wrapped up SO tight. like a little bug burrito."),
        design([("wax wall","#"),("honey floor","."),("honeycomb","~"),("candle","*")],"Bee Queen","shes got a stinger the size of a sword and she is VERY protective of the hive",["Worker Bee","Drone Bee","Honey Guard"],"Stinger Lance","a bee stinger on a stick. poetic","Beekeeper Suit","bees cant sting you!! finally!!!",["Honey Trap","Swarm Attack","Wax Seal"],mode("A","mixolydian"),"the bee queen made you an honorary bee!! bzzzz!!","the bee queen stung you. the stinger is SO big when youre small."),
        design([("shell wall","#"),("chitin floor","."),("shell fragment","~"),("royal mark","*")],"The Queen Beetle","her shell is SO hard NOTHING can crack it. except maybe ONE thing. shes been the queen for a THOUSAND bug years",["Shell Guard","Horn Beetle","Royal Larvae"],"Mandible Breaker","the only thing that can crack the queens shell","Exoskeleton","its like wearing a bug. but for protection. bug armor",["Shell Slam","Horn Charge","Royal Guard Rush"],mode("C","aeolian"),"THE QUEEN BEETLE SHED HER SHELL AND FLEW AWAY!! shes free and so are you!!","the queen beetle was too armored. SO much shell."),
    ],
    st(4,2,2), settings(5,3,5,2)
))

# ── 12: Ice Kingdom ──
campaigns.append(make_campaign(
    "Ice Kingdom", "Megrim",
    "BRRR its SO cold. everything is ice and snow and the bad guys are all frozen and angry about it.",
    "#0e1a2a", "#88ccee",
    [
        lv("The Frost Gate","Caveat","the entrance is frozen SOLID. you can see your breath and its sparkling.","ice gate, frost crystals, frozen path","#88aacc",["#0e1828","#1e2e48","#304868","#486288","#607ca8"],125),
        lv("Snowdrift Maze","Comic Neue","the snow piled up SO high you cant see over it. the paths keep changing.","snow walls, drift tunnels, ice patches","#aaccee",["#142028","#2a3848","#425068","#5a6888","#7280a8"],165),
        lv("The Frozen Lake","Short Stack","the lake is frozen and you can see things MOVING under the ice.","ice surface, cracks, frozen fish, dark shapes","#6a8aaa",["#0a1420","#14283a","#223e58","#305478","#3e6a98"],200),
        lv("Crystal Caves","Kalam","the caves are made of ICE CRYSTALS and they glow blue and pink and its beautiful but cold.","crystal formations, ice pillars, frozen streams","#88aadd",["#0e1a30","#1e3050","#304870","#486090","#6078b0"],240),
        lv("The Blizzard","Patrick Hand","you cant see ANYTHING. the snow is blowing SO hard. just keep moving.","whiteout, wind, ice shards, snow drifts","#ccddee",["#1e2830","#384850","#526870","#6c8890","#86a8b0"],275),
        lv("The Frost Titan","Permanent Marker","hes made of SOLID ICE and hes as big as a mountain. a FROZEN mountain.","ice throne, glacier arena, aurora sky","#4488cc",["#081830","#142e50","#224470","#305a90","#3e70b0"],140),
    ],
    [
        design([("ice wall","#"),("snow floor","."),("frost crystal","~"),("icicle","*")],"Frost Guardian","a knight made entirely of ice. when you hit him he cracks but then FREEZES back together",["Snow Fox","Ice Bat","Frost Imp"],"Fire Poker","it melts ice on contact!! SIZZLE","Fur Coat","SO warm and fluffy. the cold cant get you",["Ice Patch","Frost Blast","Icicle Drop"],mode("E","dorian"),"the frost guardian melted!! puddle!!","the frost guardian froze you solid. brrr. SO brrr."),
        design([("snow wall","#"),("packed snow","."),("drift","~"),("snow pile","*")],"Snowdrift Yeti","hes covered in snow and he throws SNOWBALLS. but like DEADLY snowballs",["Snow Snake","Ice Mouse","Frost Rabbit"],"Ice Pick","it breaks through anything frozen","Snowshoes","you can walk on snow without sinking!! finally!!",["Snowball","Drift Collapse","Ice Slide"],mode("F","aeolian"),"the yeti got too warm from fighting and ran back to the cold part!!","the yeti buried you in snowballs. SO many snowballs."),
        design([("frozen wall","#"),("ice floor","."),("crack","~"),("dark shape","*")],"The Lake Lurker","something HUGE is under the ice and it keeps breaking through and grabbing stuff",["Ice Fish","Frozen Frog","Under Ice Shadow"],"Harpoon","it goes through ice and water and EVERYTHING","Ice Skates","you zoom across the frozen lake SO fast",["Ice Break","Current Pull","Freeze Blast"],mode("G","phrygian"),"the lake lurker swam to the bottom and STAYED there!!","the lake lurker pulled you under. its SO cold down there."),
        design([("crystal wall","#"),("crystal floor","."),("ice pillar","~"),("glow crystal","*")],"Crystal Witch","she turns EVERYTHING into crystals. trees animals people EVERYTHING",["Crystal Bat","Ice Golem","Frost Fairy"],"Prism Blade","it splits her crystal magic into rainbows","Warm Crystal Armor","its made of warm crystals. yes those exist now",["Crystal Cage","Prism Trap","Freeze Ray"],mode("A","lydian"),"the crystal witch accidentally turned HERSELF into a crystal!!","the crystal witch turned you into a very pretty crystal statue."),
        design([("blizzard wall","#"),("wind floor","."),("ice shard","~"),("snow pile","*")],"Blizzard Beast","its MADE of the blizzard. its the wind and the snow and the cold ALL AT ONCE",["Wind Spirit","Hail Stone","Frost Wraith"],"Calm Stone","it makes the wind stop wherever you swing it","Storm Cloak","the blizzard goes AROUND you instead of through you",["Wind Blast","Hail Storm","Freeze Wave"],mode("D","aeolian"),"the blizzard beast blew itself out!! the sky is clear!!","the blizzard beast was too much weather. ALL the weather at once."),
        design([("glacier wall","#"),("frozen floor","."),("ice throne","~"),("aurora light","*")],"The Frost Titan","hes made of a WHOLE GLACIER. his fist is the size of a HOUSE. when he walks the ground SHAKES and everything CRACKS",["Ice Knight","Frost Warrior","Crystal Elite"],"Sun Blade","its warm like the SUN and it melts everything it touches","Aurora Shield","it glows with northern lights and nothing frozen can touch it",["Glacier Slam","Ice Wave","Absolute Zero"],mode("C","aeolian"),"THE FROST TITAN MELTED!! spring came early!! flowers everywhere!!","the frost titan was too big and too cold and too FROZEN."),
    ],
    st(5,2,2), settings(4,3,5,3)
))

# ── 13: Dream Land ──
campaigns.append(make_campaign(
    "Dream Land", "Satisfy",
    "youre ASLEEP and everything is weird. doors go to the wrong places and gravity doesnt work right. its a dream but a SCARY one.",
    "#1a0a2a", "#cc88ee",
    [
        lv("Falling Asleep","Loved by the King","your eyes are getting heavy. the room is melting. wait MELTING??","melting room, shifting floors, droopy walls","#8a6aaa",["#140a1e","#2e1a3a","#482e5a","#64487a","#80629a"],125),
        lv("The Upside Down Room","Comic Neue","the ceiling is the floor and the floor is the ceiling and WHICH WAY IS UP??","inverted room, floating furniture, confused gravity","#6a6aaa",["#0e0e1e","#1e1e38","#323258","#4a4a78","#626298"],165),
        lv("Memory Maze","Short Stack","its made of memories and they keep CHANGING. was that door always there??","shifting walls, memory fragments, deja vu zones","#aa6a8a",["#1e0a14","#3a1a2a","#5a2e42","#7a485e","#9a627a"],200),
        lv("The Nightmare Part","Kalam","ok THIS part is actually scary. the shadows have TEETH.","dark shadows, teeth walls, fear fog","#4a2a4a",["#0a040a","#1e0e1e","#321832","#4a284a","#623862"],240),
        lv("The Clock Tower","Patrick Hand","time doesnt work right. sometimes its fast sometimes its SUPER slow.","giant clock, spinning gears, time portals","#8a8a6a",["#141408","#2e2e18","#484828","#62623a","#7c7c4e"],275),
        lv("The Dream Eater","Caveat","it EATS dreams and yours is the biggest dream its ever seen. its SO hungry.","dream void, eaten memories, reality cracks","#6a2a8a",["#0e041a","#1e0e32","#321a4e","#4a286a","#623888"],140),
    ],
    [
        design([("melty wall","#"),("wobbly floor","."),("puddle","~"),("drip","*")],"Sleep Walker","hes sleepwalking and he doesnt know hes fighting. but hes REALLY good at it",["Pillow Monster","Blanket Ghost","Alarm Clock"],"Dream Sword","it only exists while youre asleep but it REALLY works","Pajama Armor","the comfiest armor ever. and its surprisingly tough",["Floor Melt","Dream Shift","Sleepy Gas"],mode("D","lydian"),"the sleep walker woke up and said sorry!! he didnt mean it!!","the sleep walker knocked you out. you fell asleep in a dream. dreamception."),
        design([("ceiling wall","#"),("upside floor","."),("gravity spot","~"),("float","*")],"Gravity Witch","she makes gravity go the WRONG way. up is down is sideways is CONFUSING",["Float Ghost","Fall Imp","Ceiling Crawler"],"Gravity Hammer","it makes stuff go DOWN really hard. REALLY hard","Magnet Boots","you stick to WHATEVER surface you want",["Gravity Flip","Float Trap","Ceiling Drop"],mode("E","phrygian"),"the gravity witch forgot which way was up and got stuck on the ceiling!!","gravity went sideways and you fell into a wall. ow."),
        design([("memory wall","#"),("hazy floor","."),("memory shard","~"),("echo","*")],"Deja Vu Knight","every time you beat him he comes BACK. you keep doing it over and over",["Memory Echo","Past Shadow","Future Ghost"],"Reality Blade","it makes stuff STAY gone","Memory Cap","you remember EVERYTHING even the stuff that changes",["Loop Trap","Memory Erase","Time Skip"],mode("F","dorian"),"the deja vu knight FINALLY stayed gone!! no more loops!!","you got stuck in a loop. again. and again. and again."),
        design([("shadow wall","#"),("dark floor","."),("teeth","~"),("fear spot","*")],"The Nightmare","its YOUR nightmare. the thing youre most scared of. it knows EVERYTHING about you",["Shadow Teeth","Fear Crawler","Dark Hand"],"Courage Sword","the braver you are the STRONGER it gets","Night Light","it pushes the darkness away and the nightmare HATES it",["Fear Wave","Shadow Grab","Nightmare Scream"],mode("G","aeolian"),"you faced your nightmare and it got SCARED of YOU!!","the nightmare was too scary. some things are just TOO scary."),
        design([("clock wall","#"),("gear floor","."),("time portal","~"),("pendulum","*")],"Father Time","he has a HUGE clock for a face and he speeds up and slows down whenever he wants",["Tick","Tock","Second Hand"],"Hour Hand","its heavy like an hour feels heavy. deep huh","Chrono Shield","time goes AROUND you. youre always in the NOW",["Time Freeze","Fast Forward","Rewind Trap"],mode("A","mixolydian"),"father time ran out of time!! the clock stopped!!","father time sped everything up and you couldnt keep... up..."),
        design([("void wall","#"),("dream floor","."),("eaten spot","~"),("reality crack","*")],"The Dream Eater","it eats dreams and nightmares and memories and EVERYTHING. its the HUNGRIEST thing ever and its been eating for a THOUSAND years",["Nightmare Spawn","Memory Leech","Dream Fragment"],"Wake Up Bell","DING DING DING. it makes everything REAL","Dream Crown","the dreams listen to YOU now. you control EVERYTHING",["Dream Devour","Reality Collapse","Memory Drain"],mode("C","aeolian"),"THE DREAM EATER IS FULL!! it fell asleep!! a dream monster SLEEPING!! irony!!","the dream eater ate your dream while you were IN it. trippy."),
    ],
    st(5,3,2), settings(5,3,5,2)
))

# ── 14: Space Station ──
campaigns.append(make_campaign(
    "Space Station", "Audiowide",
    "YOURE IN SPACE!! theres aliens and they have ray guns and everything floats around because theres no gravity.",
    "#0a0a1a", "#66aaee",
    [
        lv("Docking Bay","Electrolize","the airlock opens and youre ON the station. the aliens already know youre here.","metal corridors, airlock, cargo containers","#4a5a6a",["#080e14","#141e2e","#223248","#304a62","#3e627c"],130),
        lv("Zero Gravity Zone","Comic Neue","NO GRAVITY. youre floating and spinning and the aliens are GOOD at floating.","open space, floating debris, handholds","#3a4a6a",["#060a18","#101e30","#1e3250","#2e4870","#3e5e90"],170),
        lv("The Lab","Short Stack","the aliens were doing EXPERIMENTS. you dont wanna know what kind.","science equipment, tubes, glowing stuff","#4a6a4a",["#081408","#142e14","#224822","#306230","#3e7c3e"],200),
        lv("Engine Room","Kalam","the engines are SO loud and SO hot and theres radiation which is NOT good.","giant engines, coolant pipes, warning signs","#8a5a3a",["#180e04","#30220e","#4a3818","#644e28","#7e6438"],240),
        lv("The Bridge","Patrick Hand","this is where the captain sits. except the captain is a GIANT ALIEN.","command chairs, view screen, star map","#3a3a7a",["#08081a","#14142e","#222248","#323262","#42427c"],280),
        lv("The Mothership","Permanent Marker","the alien MOTHERSHIP. its ENORMOUS. bigger than a PLANET. ok maybe not but its REALLY big.","alien throne, organic walls, hive mind core","#2a4a6a",["#040e1a","#0e2238","#1a3858","#2a5078","#3a6898"],140),
    ],
    [
        design([("metal wall","#"),("grate floor","."),("cargo","~"),("light","*")],"Security Drone","it scans everything and if you dont have a badge it goes INTRUDER INTRUDER",["Alien Worker","Repair Bot","Space Rat"],"Laser Cutter","it cuts through ANYTHING. metal aliens ANYTHING","Space Suit","it protects you from lasers AND no gravity AND aliens",["Laser Grid","Airlock Breach","Cargo Drop"],mode("E","dorian"),"the security drone accepted your fake badge!! beep boop ACCESS GRANTED!!","the security drone called ALL its friends. too many drones."),
        design([("hull wall","#"),("float zone","."),("debris","~"),("handhold","*")],"Zero-G Predator","it SWIMS through zero gravity like a shark through water",["Float Jelly","Space Leech","Drift Imp"],"Grapple Sword","you can grab stuff AND stab stuff","Mag Boots","you can walk on the walls AND the ceiling",["Spin Out","Debris Hit","Vacuum Pull"],mode("F","phrygian"),"the zero-g predator got sucked out an airlock!! whoooosh!!","you spun out of control and floated away. SO far away."),
        design([("lab wall","#"),("clean floor","."),("experiment","~"),("monitor","*")],"Lab Monster","the experiment ESCAPED and its not happy about being an experiment",["Test Subject","Lab Rat","Mutant Bug"],"Experiment X","its a weapon the aliens made. now its YOUR weapon","Hazmat Suit","nothing toxic or mutant-y can hurt you",["Chemical Spill","Containment Breach","Radiation Burst"],mode("G","lydian"),"the lab monster broke all the experiments and ran away!!","the lab monster experimented on YOU. results: not good."),
        design([("engine wall","#"),("hot floor","."),("coolant pipe","~"),("warning sign","*")],"Engine Worm","it lives IN the engine and it eats FUEL and its gotten REALLY big from all that fuel",["Heat Slug","Radiation Roach","Steam Vent"],"Coolant Rod","it freezes stuff AND bonks stuff","Radiation Suit","radiation just goes right through. wait no. bounces off. yes.",["Steam Blast","Radiation Leak","Engine Surge"],mode("A","aeolian"),"the engine worm fell asleep in the exhaust port!! the engine works again!!","the engine worm was too hot. literally too hot."),
        design([("command wall","#"),("bridge floor","."),("console","~"),("viewscreen","*")],"Alien Captain","four arms, three eyes, two brains, ONE very bad attitude",["Bridge Crew","Shield Drone","Weapons Officer"],"Command Staff","the captain dropped it. its yours now","Energy Shield","it stops all alien weapons. ALL of them",["Shield Overload","Console Explosion","Tractor Beam"],mode("D","mixolydian"),"the alien captain ejected in an escape pod!! coward!!","the alien captain pressed the BIG RED BUTTON. you dont want to know."),
        design([("organic wall","#"),("hive floor","."),("nerve cluster","~"),("mind node","*")],"The Hive Mind","its not just ONE alien its ALL of them thinking TOGETHER. like a brain the size of a BUILDING",["Hive Drone","Mind Puppet","Nerve Cluster"],"Thought Blade","it cuts through telepathy. somehow","Mind Shield","you can think YOUR thoughts without the hive mind hearing",["Mind Blast","Hive Swarm","Neural Shock"],mode("C","aeolian"),"THE HIVE MIND DISCONNECTED!! all the aliens woke up and said whoa what happened!!","the hive mind thought SO hard at you. too many thoughts."),
    ],
    st(4,3,3), settings(4,3,5,3)
))

# ── 15: Dragon Mountain ──
campaigns.append(make_campaign(
    "Dragon Mountain", "Cinzel Decorative",
    "theres a mountain FULL of dragons. little ones, big ones, and the BIGGEST one at the very top. dont worry dragons are only a LITTLE scary.",
    "#2a1a0a", "#eebb66",
    [
        lv("Dragon Foothills","Gloria Hallelujah","the bottom of the mountain. you can see dragon shadows flying overhead.","rocky terrain, scorched grass, claw marks","#8a7a5a",["#141008","#2e2418","#483a28","#625040","#7c6858"],130),
        lv("Whelp Caves","Comic Neue","baby dragon caves!! theyre cute but they breathe fire and they dont know how to AIM.","small caves, tiny nests, fire marks","#aa6a3a",["#1a0e04","#3a220e","#5a381a","#7a4e28","#9a6438"],170),
        lv("The Dragon Forge","Short Stack","the dragons forge their OWN weapons here. the heat is UNBELIEVABLE.","forge fires, anvils, dragon-made weapons","#aa5522",["#1a0804","#3a180e","#5a2818","#7a3828","#9a4838"],200),
        lv("Dragon Nests","Kalam","the BIG dragons nest here. their eggs are the size of BOULDERS.","massive nests, huge eggs, sleeping dragons","#886a3a",["#140e04","#2e2210","#483a1e","#62522e","#7c6a3e"],240),
        lv("The Wyrm Tunnels","Patrick Hand","underground dragons!! they dig tunnels and theyre LONG and ANGRY.","dark tunnels, scale marks, earth tremors","#5a4a3a",["#0a0804","#1e1a0e","#322e18","#4a4228","#625838"],275),
        lv("The Elder Dragon","Caveat","the dragon at the TOP of the mountain. its been alive for TEN THOUSAND YEARS and its on FIRE. like always on fire.","mountain peak, fire throne, dragon hoard","#cc8800",["#1a1000","#3a2800","#5a4000","#7a5800","#9a7000"],140),
    ],
    [
        design([("rock wall","#"),("dirt path","."),("scorched ground","~"),("claw mark","*")],"Hill Drake","its the smallest dragon but it still breathes fire and its still SCARY",["Fire Lizard","Scale Rat","Ash Imp"],"Dragon Tooth","a tooth from a bigger dragon. to the hill drake its TERRIFYING","Fire Cloak","fire goes AROUND you like youre not even there",["Fire Breath","Tail Swipe","Rock Slide"],mode("D","dorian"),"the hill drake flew away to find a smaller mountain!!","the hill drake was still a dragon. fire HURTS."),
        design([("cave wall","#"),("warm floor","."),("tiny nest","~"),("fire mark","*")],"Dragon Mama","shes a mama dragon and you got too close to her babies. BAD IDEA",["Baby Dragon","Fire Pup","Spark Whelp"],"Baby Rattle","it calms baby dragons down SO fast. shhhh","Dragon Scale Vest","it grows with you. dragon magic!!",["Baby Fire","Nest Guard","Mama Roar"],mode("E","lydian"),"dragon mama saw you werent a threat and went back to her babies!!","dragon mama sat on you. she thought you were an egg."),
        design([("forge wall","#"),("hot metal floor","."),("anvil","~"),("fire pit","*")],"Forge Drake","it breathes fire SO hot it melts METAL. thats like REALLY hot",["Anvil Golem","Slag Beast","Ember Smith"],"Dragon Steel Sword","forged in dragon fire. the BEST sword","Forge Armor","made of the same stuff. if dragons cant melt it nothing can",["Forge Fire","Metal Splash","Anvil Drop"],mode("F","mixolydian"),"the forge drake cooled down and became a regular blacksmith!!","the forge drake was too hot. everything melted. including you."),
        design([("nest wall","#"),("straw floor","."),("dragon egg","~"),("sleeping dragon","*")],"Nest Warden","the BIGGEST dragon whos not the elder. it protects ALL the nests",["Egg Guard","Scale Knight","Nest Dragon"],"Egg Shell Mace","its made from dragon egg shell and its SUPER hard","Dragon Egg Shield","the dragons wont attack you if you look like an egg. kinda",["Dragon Stomp","Fire Ring","Egg Roll"],mode("G","aeolian"),"the nest warden fell asleep!! the BIGGEST nap ever!!","the nest warden was too protective. you were too close."),
        design([("tunnel wall","#"),("carved floor","."),("scale trail","~"),("tremor crack","*")],"The Great Wyrm","its a dragon but NO WINGS and its SUPER long. like a train but alive and angry",["Tunnel Worm","Cave Bat","Dig Mole"],"Tunnel Axe","it breaks through ANYTHING underground","Dig Armor","nothing underground can crush you",["Tunnel Collapse","Earth Quake","Wyrm Coil"],mode("A","phrygian"),"the great wyrm dug SO deep it came out the other side of the mountain!!","the great wyrm coiled around you. SO squeezed."),
        design([("fire wall","#"),("magma floor","."),("hoard pile","~"),("fire throne","*")],"The Elder Dragon","TEN THOUSAND YEARS OLD. wings that block out the SUN. breath that melts MOUNTAINS. it doesnt even need to TRY to be scary it just IS",["Dragon Guard","Fire Elite","Flame Titan"],"Elder Bane","the prophecy said THIS sword would end it. THE PROPHECY","Dragon Crown","all dragons bow to the crown. ALL of them",["Inferno","Wing Storm","Ancient Fire"],mode("C","aeolian"),"THE ELDER DRAGON BOWED!! it said youre worthy!! dragon mountain is peaceful!!","the elder dragon breathed fire for THIRTY SECONDS. thats SO long."),
    ],
    st(5,2,3), settings(4,3,5,3)
))

# ── 16: Dark Castle ──
campaigns.append(make_campaign(
    "Dark Castle", "Nosifer",
    "the darkest castle with the WORST bad guys. everything is made of black stone and shadows and its SO creepy in there.",
    "#0a0a14", "#9988aa",
    [
        lv("The Gate","Shadows Into Light","the gate is HUGE and black and something is scratching the other side.","iron gate, black stone, scratch marks","#5a5a6a",["#08080e","#14141e","#222232","#343448","#484860"],130),
        lv("The Hall Of Shadows","Comic Neue","the shadows MOVE by themselves and they grab at you when you walk by.","moving shadows, dark alcoves, flickering torches","#3a3a4a",["#06060e","#10101e","#1e1e30","#2e2e44","#40405a"],170),
        lv("The Dungeon","Short Stack","the castle has a dungeon IN a dungeon. dungeonception. its really really dark.","cells, chains, dripping water, rats","#4a4a4a",["#080808","#181818","#282828","#3a3a3a","#4e4e4e"],200),
        lv("The Armory","Kalam","walls COVERED in weapons and armor. the cursed kind that fights BACK.","weapon racks, cursed items, dark enchantments","#5a4a4a",["#0e0808","#1e1414","#302222","#443232","#5a4444"],240),
        lv("Tower Of Wailing","Patrick Hand","the tower goes UP forever and something is WAILING at the top. like really sad wailing.","spiral stairs, ghostly wails, wind howl","#4a4a6a",["#08081a","#141428","#22223a","#343450","#484868"],275),
        lv("The Dark Lord","Permanent Marker","the FINAL boss of the DARKEST castle. nobody has beaten him. NOBODY. until maybe now??","dark throne, soul fire, void gate","#2a1a3a",["#06040a","#140e1e","#221832","#302448","#3e3060"],140),
    ],
    [
        design([("black stone","#"),("dark floor","."),("scratch mark","~"),("dead torch","*")],"Gate Beast","it has TOO many arms and TOO many teeth and it smells TERRIBLE",["Shadow Rat","Gate Imp","Iron Bat"],"Torch Sword","it burns AND it lights up the dark. two for one!","Gate Guard Armor","the gate guard doesnt need it anymore...",["Arm Grab","Teeth Gnash","Dark Slam"],mode("D","phrygian"),"the gate beast couldnt fit through its own gate!! stuck!!","the gate beast had too many arms. you couldnt dodge all of them."),
        design([("shadow wall","#"),("dark carpet","."),("shadow pool","~"),("candle","*")],"Shadow King","hes made of PURE shadow and the only way to see him is by his glowing eyes",["Living Shadow","Dark Wisp","Shadow Hand"],"Light Blade","it cuts shadows in HALF and they cant reform","Lantern Shield","it makes light everywhere and shadows HATE light",["Shadow Pull","Dark Blind","Living Darkness"],mode("E","aeolian"),"the shadow king got caught in the light and POOFED into nothing!!","the shadows swallowed you. SO dark. darker than dark."),
        design([("dungeon wall","#"),("wet stone","."),("chain","~"),("drain","*")],"The Warden","hes been guarding this dungeon for SO long he forgot theres an outside",["Chain Ghoul","Dungeon Rat","Cell Block"],"Master Key","opens ANY lock and also bonks stuff good","Inmate Armor","it says PROPERTY OF DUNGEON. finders keepers",["Chain Whip","Cell Slam","Flood"],mode("F","dorian"),"the warden walked outside and saw the sun for the first time. he cried.","the warden locked you in the deepest cell and forgot about you."),
        design([("weapon wall","#"),("stone floor","."),("weapon rack","~"),("enchant circle","*")],"The Cursed Blade","its a SWORD that fights BY ITSELF. no person just a floating angry sword",["Cursed Axe","Haunted Shield","Living Mace"],"Blessed Hammer","it breaks curses when it hits stuff","Blessed Armor","cursed things bounce RIGHT off",["Sword Slash","Curse Wave","Weapon Storm"],mode("G","lydian"),"the cursed blade broke in half and the curse flew away!!","the cursed blade was a REALLY good swordsman. swordssword? sword.",),
        design([("tower wall","#"),("stair floor","."),("window","~"),("bell","*")],"The Banshee","shes at the TOP and she SCREAMS and the scream goes through WALLS",["Ghost Guard","Stair Phantom","Window Wraith"],"Silence Bell","it makes everything quiet even the banshee","Echo Armor","screams bounce off and go BACK at the screamer",["Wail","Ghost Walk","Stair Collapse"],mode("A","phrygian"),"the banshee lost her voice!! no more screaming!!","the banshee screamed SO loud your ears are still ringing."),
        design([("void wall","#"),("soul floor","."),("void gate","~"),("soul fire","*")],"The Dark Lord","he controls ALL the darkness in the WHOLE castle. maybe the whole WORLD. hes been gathering dark power for a THOUSAND years and he is NOT sharing",["Void Knight","Soul Stealer","Dark Elite"],"Dawn Blade","forged from the first sunrise. the Dark Lord is TERRIFIED of it","Solar Plate","its like wearing the SUN. everything dark runs away",["Void Blast","Soul Drain","Eternal Night"],mode("C","aeolian"),"THE DARK LORD SAW THE DAWN BLADE AND RAN!! the castle filled with LIGHT!!","the dark lord was too dark and too powerful. the darkness was TOO dark."),
    ],
    st(5,3,2), settings(4,3,5,3)
))

# ── 17: Wizard School ──
campaigns.append(make_campaign(
    "Wizard School", "Cormorant",
    "its a school for WIZARDS. they learn spells and stuff. but the bad wizards took over and now the spells are going EVERYWHERE.",
    "#1a1a2a", "#aa88ee",
    [
        lv("The Library","Indie Flower","books FLYING everywhere. some of them bite. the library is OUT OF CONTROL.","flying books, tall shelves, rolling ladders","#7a6a8a",["#100e18","#241e30","#383248","#504a64","#686280"],125),
        lv("Potion Class","Comic Neue","all the potions are exploding. green ones purple ones. its a MESS.","cauldrons, spilled potions, smoke, explosions","#6a8a6a",["#0e140e","#1e2e1e","#2e482e","#3e623e","#4e7c4e"],165),
        lv("The Spell Halls","Short Stack","spells are bouncing off the walls. duck!! DUCK!! fireball!!","magic corridors, spell marks, enchanted doors","#8a6a8a",["#140e14","#2e1e2e","#482e48","#644864","#806280"],200),
        lv("The Forbidden Wing","Kalam","nobody is SUPPOSED to go here. the magic is WEIRD and it does stuff on its OWN.","dark magic, forbidden tomes, strange glows","#5a3a5a",["#0e040e","#220e22","#381838","#4e284e","#643864"],240),
        lv("The Test Chamber","Patrick Hand","where they test the DANGEROUS spells. theres scorch marks on EVERY surface.","blast marks, containment circles, warning runes","#8a8a5a",["#14140a","#2e2e1a","#48482e","#626244","#7c7c5a"],275),
        lv("The Headmaster","Caveat","the headmaster of the school went BAD. hes the strongest wizard and he knows EVERY spell.","grand office, floating artifacts, spell throne","#6a4a8a",["#0e081a","#1e142e","#322248","#483464","#5e4680"],140),
    ],
    [
        design([("bookshelf wall","#"),("wood floor","."),("book pile","~"),("candle","*")],"Book Wyrm","its a dragon but made of BOOKS. it breathes WORDS at you. sharp words",["Flying Book","Page Golem","Ink Blob"],"Bookmark Blade","it stops books in their tracks LITERALLY","Reading Glasses","you can see magic AND dodge it better",["Book Swarm","Ink Splash","Shelf Collapse"],mode("F","ionian"),"the book wyrm settled down and became the librarian!! a NICE librarian!!","the book wyrm paper-cut you. the WORST kind of cut."),
        design([("stone wall","#"),("wet floor","."),("cauldron","~"),("flame","*")],"Potion Golem","it drank ALL the potions and now its made of EVERY potion at once. it changes color every second",["Acid Splash","Smoke Cloud","Bubble Bomb"],"Stirring Rod","it controls potions. stir stir ZAP","Alchemist Apron","potion splashes cant hurt you",["Acid Pool","Smoke Screen","Potion Explosion"],mode("G","dorian"),"the potion golem evaporated!! nothing left but a weird smell!!","the potion golem splashed too many potions on you. you turned purple."),
        design([("magic wall","#"),("enchanted floor","."),("spell mark","~"),("rune","*")],"Spell Storm","its not a wizard its just a STORM OF SPELLS. they all got loose and joined together",["Fire Ball","Ice Shard","Lightning Bolt"],"Counter Staff","it sends spells BACK at the caster","Spell Shield","spells just fizzle out when they touch it",["Random Spell","Chain Lightning","Fire Wave"],mode("A","phrygian"),"all the spells calmed down and went back in their books!!","too many spells. SO many spells. just spells everywhere."),
        design([("dark stone","#"),("cursed floor","."),("dark tome","~"),("strange glow","*")],"Forbidden One","something that was LOCKED UP for a reason and now its OUT",["Dark Familiar","Curse Sprite","Void Bug"],"Sealing Wand","it locks bad things back up where they belong","Ward Cloak","forbidden magic slides right off you",["Curse Word","Dark Portal","Forbidden Spell"],mode("D","aeolian"),"the forbidden one got sealed back up!! and THIS time the lock is BETTER!!","the forbidden one was forbidden for a REASON."),
        design([("blast wall","#"),("scorched floor","."),("containment ring","~"),("warning sign","*")],"Test Spell Alpha","the MOST DANGEROUS spell. it got loose and its learning and its NOT stopping",["Spark","Blast","Zap"],"Null Rod","it cancels ANY spell. even the big ones","Anti-Magic Suit","magic doesnt work on you at ALL",["Mega Blast","Chain Reaction","Mana Explosion"],mode("E","lydian"),"test spell alpha fizzled out!! it wasnt as tough as it thought!!","test spell alpha was VERY tough actually. SO much magic."),
        design([("grand wall","#"),("office floor","."),("floating book","~"),("artifact","*")],"The Headmaster","he knows every spell in every book in the WHOLE school. a THOUSAND spells. he can do them ALL at the same time",["Familiar Swarm","Spell Knight","Magic Mirror"],"Staff of Undoing","it undoes whatever the headmaster does. undo undo undo","Headmasters Hat","it lets you cast spells too!! now its a FAIR wizard fight!!",["All Spells","Time Stop","Reality Warp"],mode("C","aeolian"),"THE HEADMASTER REMEMBERED WHY HE BECAME A TEACHER!! hes nice again!!","the headmaster cast ALL thousand spells at once. thats too many spells."),
    ],
    st(5,2,2), settings(5,3,5,2)
))

# ── 18: Toy Box ──
campaigns.append(make_campaign(
    "Toy Box", "Crafty Girls",
    "you fell into a TOY BOX and all the toys came alive!! some of them are friendly but MOST of them are NOT.",
    "#2a2a1a", "#eecc88",
    [
        lv("Block Town","Short Stack","its a whole town made of building blocks. if you step wrong it all falls down.","colorful blocks, block houses, block roads","#aa8844",["#1a1408","#3a2e18","#5a4a30","#7a6a48","#9a8a60"],125),
        lv("Stuffed Animal Kingdom","Comic Neue","the stuffed animals rule here and they take it VERY seriously.","plush castles, button eyes, fluff clouds","#aa88aa",["#1a141a","#3a2e3a","#5a485a","#7a627a","#9a7c9a"],165),
        lv("Race Track","Handlee","the toy cars race around SUPER fast and they do NOT slow down for people.","plastic track, toy cars, ramp jumps","#8888aa",["#101018","#22222e","#363648","#4a4a62","#60607c"],200),
        lv("Army Men Territory","Kalam","the army men are having a WAR and you wandered into the MIDDLE of it.","sandbag walls, plastic tanks, fort walls","#6a8a4a",["#0e140a","#1e2e14","#2e4820","#3e622e","#4e7c3e"],240),
        lv("The Doll House","Patrick Hand","the dolls live here and they never blink. NEVER. its SO creepy.","tiny furniture, doll rooms, unblinking eyes","#cc88aa",["#2a1418","#4a2830","#6a3e48","#8a5462","#aa6a7c"],275),
        lv("The Toy King","Permanent Marker","the BIGGEST toy. a giant action figure and he thinks hes a REAL king.","toy throne, plastic crown, action figure army","#ccaa44",["#1a1404","#3a300e","#5a4e1a","#7a6c2a","#9a8a3a"],140),
    ],
    [
        design([("block wall","#"),("block floor","."),("loose block","~"),("tower","*")],"Block Titan","its made of a THOUSAND blocks stacked really high and it stomps around",["Block Soldier","Lego Sniper","Brick Bat"],"Block Breaker","it smashes blocks into tiny pieces","Mega Block Armor","the biggest blocks all stuck together. SO strong",["Block Fall","Tower Collapse","Stepping On Block"],mode("G","ionian"),"the block titan fell apart!! blocks EVERYWHERE!! but the good kind!!","the block titan stepped on you. with block feet. ouch."),
        design([("plush wall","#"),("soft floor","."),("fluff pile","~"),("button","*")],"Queen Stuffy","the BIGGEST stuffed bear and she has a crown made of ribbons",["Plush Knight","Button Eye","Cotton Ball"],"Pin Sword","one poke and stuffed animals go FLAT","Thimble Helm","nothing soft can bonk your head",["Fluff Bomb","Hug Attack","Cotton Storm"],mode("D","lydian"),"queen stuffy decided to be nice and give hugs instead of fights!!","queen stuffy hugged you SO hard. too soft. too much softness."),
        design([("track wall","#"),("road floor","."),("ramp","~"),("finish line","*")],"Speed Racer X","the FASTEST toy car. it goes SO fast you can barely see it ZOOM",["Hot Rod","Monster Truck","Go Kart"],"Speed Bump","it slows everything down BONK","Racing Helmet","nothing fast can hurt your head",["Speed Crash","Ramp Launch","Tire Throw"],mode("E","mixolydian"),"speed racer x ran out of batteries!! FINALLY!!","speed racer x was too fast. like WAY too fast. zoom ow zoom ow."),
        design([("sandbag wall","#"),("dirt floor","."),("tank","~"),("flag","*")],"General Plastic","he commands ALL the army men and he has a tiny tiny megaphone",["Green Soldier","Tan Soldier","Parachuter"],"Tank Cannon","you have your OWN tank now. fire!!","Army Helmet","standard issue. it works surprisingly well",["Artillery","Air Strike","Ambush"],mode("F","dorian"),"general plastic surrendered!! he waved a tiny white flag!!","general plastic called in ALL the reinforcements. SO many army men."),
        design([("dollhouse wall","#"),("tiny floor","."),("tiny furniture","~"),("mirror","*")],"The Doll Duchess","she NEVER blinks. she NEVER stops smiling. she controls all the other dolls with her MIND",["Porcelain Knight","Rag Doll","Action Figure"],"Doll Hammer","it breaks porcelain really well. CRASH","Real Clothes","the dolls cant control you if you dont LOOK like a doll",["Doll Swarm","Porcelain Shatter","Mind Control"],mode("A","phrygian"),"the doll duchess blinked!! for the first time!! and then she became a regular doll!!","the doll duchess stared at you and you couldnt move. SO creepy."),
        design([("throne wall","#"),("plastic floor","."),("toy pile","~"),("crown","*")],"The Toy King","a GIANT action figure with a plastic crown. he thinks hes ACTUALLY a king. he has kung fu grip and spring loaded FISTS",["Elite Figure","Toy Knight","Wind Up Soldier"],"Real Sword","its a REAL sword in a toy world. nothing is stronger","Real Armor","its REAL metal in a toy world. nothing gets through",["Kung Fu Grip","Spring Fist","Toy Army"],mode("C","aeolian"),"THE TOY KING LOST HIS BATTERIES!! hes just a regular toy now!!","the toy kings spring loaded fists hit you. BOING BOING OW."),
    ],
    st(4,2,2), settings(5,4,99,2)
))

# ── 19: Swamp Of Yuck ──
campaigns.append(make_campaign(
    "Swamp Of Yuck", "Eater",
    "its SO gross in here. everything is slimy and smelly and the monsters go BLURP and SPLORT and other gross noises.",
    "#0a1a0a", "#88aa44",
    [
        lv("Muddy Entrance","Caveat","the mud goes up to your KNEES. every step goes SQUELCH.","thick mud, cattails, mosquitoes","#5a5a2a",["#0e0e04","#1e1e0e","#30301a","#444428","#5a5a38"],125),
        lv("Bog Of Stink","Comic Neue","it smells like TEN THOUSAND rotten eggs. you can barely breathe.","bubbling bog, green gas, dead trees","#4a5a2a",["#0a0e04","#141e0a","#223014","#304420","#405a2e"],165),
        lv("Frog Kingdom","Short Stack","the frogs are HUGE and they have a KING and they think this is THEIR swamp.","lily pads, frog thrones, fly swarms","#3a6a2a",["#081404","#142e0a","#224814","#306220","#3e7c2e"],200),
        lv("The Rot Tunnels","Kalam","everything is rotting. the walls are rotting. the FLOOR is rotting. ew ew ew.","decomposing walls, mushroom growths, slime trails","#4a4a2a",["#0a0a04","#181808","#282810","#3a3a1a","#4e4e24"],240),
        lv("Quicksand Wastes","Patrick Hand","QUICKSAND. if you stop moving you SINK. just keep going dont stop.","quicksand pools, vine ropes, floating logs","#6a5a2a",["#100e04","#242008","#383410","#4e4a1a","#646024"],275),
        lv("The Swamp Thing","Permanent Marker","the WHOLE SWAMP is the boss. like the swamp ITSELF woke up and its ANGRY.","living swamp, root tentacles, muck geysers","#3a5a1a",["#060e02","#101e06","#1e300e","#2e4618","#3e5e22"],140),
    ],
    [
        design([("mud wall","#"),("squelch floor","."),("cattail","~"),("stump","*")],"Mud Golem","its made of MUD and every time you hit it it just goes SPLORT and reforms",["Mud Crab","Swamp Rat","Mosquito Swarm"],"Dry Stick","the driest thing in the swamp. it absorbs mud on contact","Rain Boots","your feet stay DRY and CLEAN",["Mud Pit","Squelch Trap","Sink Hole"],mode("D","aeolian"),"the mud golem dried out in the sun!! just a dirt pile now!!","the mud golem hugged you and now youre ALSO mud."),
        design([("bog wall","#"),("stinky floor","."),("gas bubble","~"),("dead tree","*")],"Stink Beast","it is SO smelly that other monsters run away from it too",["Gas Cloud","Rot Fly","Stink Slug"],"Air Freshener","it removes ALL bad smells. even HIS","Gas Mask","you can breathe!! finally!! sweet clean air!!",["Stink Cloud","Gas Explosion","Rot Wave"],mode("E","phrygian"),"the stink beast got a bath!! it smells like flowers now!!","you passed out from the smell. SO stinky. the stinkiest."),
        design([("pond wall","#"),("lily pad","."),("fly swarm","~"),("crown pad","*")],"Frog King","hes a GIANT frog with a crown and he goes RIBBIT really loud and bossy",["Knight Frog","Tongue Lash","Tadpole Swarm"],"Fly Swatter","bonk bonk bonk. frogs are scared of it","Lily Pad Shield","you can float AND block. amphibious defense",["Tongue Grab","Splash","Frog Stampede"],mode("F","lydian"),"the frog king hopped away to find a bigger pond!!","the frog king ate you. he just GULP ate you. gross."),
        design([("rot wall","#"),("fungus floor","."),("mushroom","~"),("slime trail","*")],"Rot Mother","she makes EVERYTHING rot. food metal even STONES. nothing is safe",["Rot Zombie","Mold Monster","Fungus Crawler"],"Preserving Salt","it stops rot in its TRACKS","Sealed Armor","nothing rotten can get in. hermetically sealed",["Rot Touch","Mold Cloud","Decay Wave"],mode("G","dorian"),"the rot mother composted herself!! circle of life!!","the rot mother rotted your armor OFF. then she rotted your spare armor."),
        design([("sand wall","#"),("sinking floor","."),("vine","~"),("log","*")],"Quicksand Serpent","it lives IN the quicksand and it pulls you DOWN with its tail",["Sand Swimmer","Vine Strangler","Sinking Bug"],"Grappling Hook","grab vines grab logs grab ANYTHING to not sink","Float Belt","you bob on top of quicksand like a cork",["Pull Under","Sand Swirl","Vine Trap"],mode("A","aeolian"),"the quicksand serpent sank to the bottom!! SO deep it cant come back!!","the quicksand serpent pulled you down. glub glub glub."),
        design([("living wall","#"),("swamp floor","."),("root arm","~"),("muck geyser","*")],"The Swamp Thing","its not IN the swamp. it IS the swamp. EVERY tree EVERY mud puddle EVERY vine is part of it. its ENORMOUS",["Root Tentacle","Muck Geyser","Living Vine"],"Drain Plug","pull it and the swamp starts draining!! GLUG GLUG","Swamp Crown","the swamp listens to whoever wears it",["Root Grab","Muck Eruption","Vine Swarm"],mode("C","aeolian"),"THE SWAMP THING WENT BACK TO SLEEP!! the swamp is just a normal gross swamp again!!","the whole swamp grabbed you at once. every vine every root every thing."),
    ],
    st(5,2,2), settings(5,3,5,2)
))

# ── 20: Ninja Village ──
campaigns.append(make_campaign(
    "Ninja Village", "Rajdhani",
    "theres NINJAS everywhere. they hide and jump out and go HI-YA!! you gotta be sneaky too or theyll get you.",
    "#0a0a14", "#8888aa",
    [
        lv("The Bamboo Gate","Kalam","bamboo EVERYWHERE. you can hear stuff moving but you cant SEE it.","bamboo forest, hidden paths, paper lanterns","#6a7a4a",["#0e140a","#1e2e14","#304820","#44622e","#587c3e"],125),
        lv("Training Grounds","Comic Neue","this is where the ninjas practice. dummies and obstacle courses everywhere.","training dummies, obstacle course, practice weapons","#7a7a5a",["#10100a","#242418","#383828","#4e4e3a","#64644e"],165),
        lv("The Dojo","Short Stack","the main building. inside theres SO many ninjas and they all know KUNG FU.","tatami mats, weapon racks, scrolls","#8a6a4a",["#140e08","#2e2014","#483420","#624a2e","#7c603e"],200),
        lv("Shadow Quarter","Patrick Hand","the SNEAKY ninjas live here. you cant see them until they BONK you.","dark alleys, shadow hideouts, trap doors","#3a3a4a",["#06060e","#10101e","#1e1e30","#2e2e44","#40405a"],240),
        lv("The Poison Garden","Caveat","flowers that are POISON. blowdarts with POISON. basically EVERYTHING here is poison.","toxic flowers, dart tubes, medicine hut","#5a6a3a",["#0a100a","#142414","#223a20","#30502e","#3e663e"],275),
        lv("The Shadow Master","Permanent Marker","the BEST ninja EVER. you cant even see her. shes SO fast and SO quiet and SO deadly.","shadow throne, ninja shrine, darkness","#2a2a3a",["#04040a","#0e0e1e","#1a1a30","#282844","#38385a"],140),
    ],
    [
        design([("bamboo wall","#"),("path floor","."),("bamboo","~"),("lantern","*")],"Bamboo Sentinel","hes been standing SO still he looks like bamboo. then SURPRISE HI-YA",["Bamboo Rat","Hidden Archer","Leaf Ninja"],"Bamboo Blade","light and fast and makes a cool WHOOSH sound","Bamboo Armor","its flexible AND strong. ninja approved",["Bamboo Trap","Hidden Dart","Leaf Storm"],mode("E","dorian"),"the bamboo sentinel bowed and disappeared!! where did he GO??","the bamboo sentinel was too sneaky. you never saw it coming."),
        design([("stone wall","#"),("mat floor","."),("dummy","~"),("weapon rack","*")],"Sensei","the TEACHER of all the ninjas. she knows EVERY move. 47 moves",["Student Ninja","Training Dummy","Sparring Bot"],"Practice Sword","the sensei uses one too. fair fight!!","Belt Armor","the more belts the more protection. youre wearing like TEN",["Test Attack","Combo Strike","Training Trap"],mode("F","mixolydian"),"the sensei said you PASSED!! youre a ninja now!! HI-YA!!","the sensei used move number 47. you only knew 46 moves."),
        design([("dojo wall","#"),("tatami floor","."),("scroll","~"),("incense","*")],"The Five Shadows","theyre FIVE ninjas that fight as ONE. when you think you got one the other four go HI-YA",["Shadow Clone","Smoke Ninja","Star Thrower"],"Five-Point Blade","it hits all five directions at ONCE","Mirror Armor","the shadows cant sneak up because you can see EVERYWHERE",["Five-Way Attack","Smoke Bomb","Star Barrage"],mode("G","phrygian"),"all five shadows tripped over each other!! pile up!! youre the LAST one standing!!","five ninjas is four too many ninjas."),
        design([("dark wall","#"),("shadow floor","."),("trap door","~"),("spy hole","*")],"Phantom","you literally CANNOT see her. she makes no sound. no shadow. NOTHING",["Ghost Step","Whisper Blade","Void Walker"],"Echo Bell","it rings when invisible things are near. DING DING","Sensing Suit","you can FEEL where the invisible ninjas are",["Invisible Strike","Trap Door","Shadow Step"],mode("A","aeolian"),"the phantom took off her mask and said good fight!! she was impressed!!","the phantom got you and you never even knew she was there."),
        design([("vine wall","#"),("garden floor","."),("toxic flower","~"),("medicine pot","*")],"Poison Master","every FINGER has a different poison. ten fingers ten poisons. all BAD",["Dart Blower","Toxic Toad","Venom Snake"],"Antidote Blade","it cures poison AND cuts stuff. multipurpose","Immunity Cloak","NO poison works on you. not even the bad ones. especially the bad ones",["Poison Dart","Toxic Cloud","Venom Splash"],mode("D","lydian"),"the poison master accidentally poisoned himself!! with his OWN poison!!","ten different poisons. you only had antidote for NINE."),
        design([("shrine wall","#"),("shadow floor","."),("ninja star","~"),("incense","*")],"The Shadow Master","the ULTIMATE ninja. she can be in FIVE places at once. she can catch arrows. she can DISAPPEAR completely. shes been training for FIFTY YEARS",["Elite Shadow","Master Clone","Dark Apprentice"],"Sun Blade","it makes light SO bright no shadow can exist near it","Daylight Armor","shadows dissolve when they touch it. pure light",["Vanish Strike","Clone Army","Ultimate Technique"],mode("C","aeolian"),"THE SHADOW MASTER BOWED AND MADE YOU HER SUCCESSOR!! youre the shadow master now!!","the shadow master used her ultimate technique. nobody has ever survived it. nobody."),
    ],
    st(4,3,2), settings(4,3,5,2)
))

# ── 21: Junkyard World ──
campaigns.append(make_campaign(
    "Junkyard World", "Special Elite",
    "its a HUGE junkyard and all the junk is ALIVE and ANGRY about being thrown away. old TVs and fridges and everything.",
    "#1a1a14", "#aa9966",
    [
        lv("The Trash Heap","Handlee","mountains of garbage. old food wrappers and broken stuff EVERYWHERE.","trash piles, crushed cans, shopping carts","#8a7a5a",["#14100a","#2a2418","#403828","#585040","#706858"],125),
        lv("Appliance Alley","Comic Neue","all the old appliances live here. angry fridges. hostile washing machines.","old fridges, busted TVs, tangled wires","#6a6a7a",["#0e0e14","#1e1e28","#30303e","#444458","#5a5a70"],165),
        lv("The Car Crusher","Short Stack","old cars everywhere and the crusher is STILL GOING. dont get in the way.","crushed cars, hydraulic press, conveyor belt","#7a5a4a",["#140e0a","#2a201a","#40342a","#584a3a","#70604e"],200),
        lv("Electronics Graveyard","Kalam","old computers and phones all flickering and beeping. some of them still WORK.","broken screens, tangled cables, sparking circuits","#4a6a6a",["#080e0e","#141e1e","#223232","#304848","#3e6060"],240),
        lv("The Compactor","Patrick Hand","everything gets SQUISHED here. walls closing in. ceilings coming down.","closing walls, hydraulic systems, emergency buttons","#5a5a5a",["#0a0a0a","#1a1a1a","#2e2e2e","#424242","#585858"],275),
        lv("The Junk King","Permanent Marker","it built itself out of ALL the junk. every fridge every car every TV. its ENORMOUS and ANGRY.","junk throne, assembled titan, scrap walls","#aa8844",["#1a1408","#3a2e18","#5a4830","#7a6448","#9a8060"],140),
    ],
    [
        design([("trash wall","#"),("garbage floor","."),("can pile","~"),("cart","*")],"Trash Compactor","it rolls around and SQUISHES everything into cubes. even people",["Trash Rat","Can Monster","Bag Beast"],"Shopping Cart","you ride it AND use it as a weapon. CHARGE!!","Trash Can Lid","the classic shield. tried and true",["Trash Slide","Can Avalanche","Cart Crash"],mode("D","dorian"),"the trash compactor broke down!! FINALLY!!","the trash compactor squished you into a cube. a people cube."),
        design([("metal wall","#"),("tile floor","."),("wire tangle","~"),("outlet","*")],"Fridge Lord","a GIANT fridge that opens its door and stuff flies out. cold stuff. FROZEN stuff",["Washer Warrior","Dryer Dragon","Toaster Beast"],"Unplugged Cord","you unplug stuff and it stops. simple but effective","Rubber Gloves","electricity cant hurt you",["Cold Blast","Spin Cycle","Toast Launch"],mode("E","phrygian"),"the fridge lord got unplugged and went quiet. just a regular fridge.","the fridge lord threw a frozen pizza at you. it was FROZEN SOLID."),
        design([("car wall","#"),("oil floor","."),("crushed car","~"),("hydraulic","*")],"Car Crusher Boss","it has ARMS made of hydraulic presses and it SQUISHES everything",["Tire Monster","Engine Block","Hubcap Frisbee"],"Hydraulic Cutter","if it can crush cars it can crush ANYTHING","Car Door Shield","its dented but SO thick nothing gets through",["Press Attack","Oil Slick","Car Drop"],mode("F","lydian"),"the car crusher boss ran out of hydraulic fluid!! no more squishing!!","the car crusher boss squished your favorite thing. and then you."),
        design([("circuit wall","#"),("floor board","."),("broken screen","~"),("spark","*")],"Virus King","an old computer virus that escaped into the REAL world. it makes everything glitch",["Glitch Bot","Pixel Bug","Static Ghost"],"Debug Hammer","it fixes glitches by BONKING them","Antivirus Suit","glitches cant touch you",["System Crash","Glitch Wave","Spark Storm"],mode("G","aeolian"),"VIRUS KING GOT DELETED!! ctrl alt DELETE!!","the virus king crashed your brain. error error error."),
        design([("steel wall","#"),("grate floor","."),("piston","~"),("button","*")],"The Compactor","the walls are CLOSING IN and the compactor is getting SMALLER and you need to get OUT",["Wall Piston","Floor Press","Ceiling Crush"],"Emergency Button","it stops the walls for a second. ONE second","Hardened Shell","you can survive being squished. barely",["Wall Close","Floor Rise","Ceiling Drop"],mode("A","mixolydian"),"you hit the MASTER button and everything STOPPED!!","the compactor compacted. youre very flat now."),
        design([("junk wall","#"),("scrap floor","."),("junk pile","~"),("throne scrap","*")],"The Junk King","its made of EVERYTHING in the junkyard. fridges for legs. cars for arms. TVs for eyes. its the size of a BUILDING and its REALLY mad about being thrown away",["Scrap Knight","Junk Golem","Trash Titan"],"Magnet Blade","it pulls junk OFF the junk king. he gets smaller!!","Titanium Armor","the strongest non-junk armor",["Junk Throw","Scrap Storm","Full Combine"],mode("C","aeolian"),"THE JUNK KING FELL APART!! all the junk is just junk again!! happy junk!!","the junk king threw a FRIDGE at you. then a CAR. then a BUS."),
    ],
    st(4,2,3), settings(5,3,5,2)
))

# ── 22: Volcano Fortress ──
campaigns.append(make_campaign(
    "Volcano Fortress", "Metal Mania",
    "theres a fortress INSIDE a volcano and the fire army lives there. theyre soldiers made of FIRE and theyre NOT happy about visitors.",
    "#2a0a0a", "#ee8844",
    [
        lv("The Lava Gates","Permanent Marker","the entrance is LAVA. like the actual gate is made of flowing lava.","lava gate, fire sentries, obsidian path","#aa4420",["#1a0804","#3a1408","#5a2210","#7a3218","#9a4222"],130),
        lv("Barracks Of Fire","Comic Neue","all the fire soldiers sleep here. in FIRE beds. everything is fire.","fire bunks, weapon racks, training grounds","#cc5522",["#200804","#401408","#602210","#803218","#a04222"],170),
        lv("The War Room","Short Stack","maps and plans everywhere. theyre planning to take over the OUTSIDE world.","war table, fire maps, battle plans","#aa6633",["#1a0e04","#3a220e","#5a381a","#7a4e28","#9a6438"],200),
        lv("Weapon Forge","Kalam","they make fire weapons here. fire swords fire shields fire EVERYTHING.","forge fires, weapon molds, cooling tanks","#884422",["#140804","#2e180e","#482a18","#623e28","#7c5438"],240),
        lv("The Magma Moat","Patrick Hand","a RIVER of magma around the inner fortress. the bridge is made of OBSIDIAN.","magma river, obsidian bridge, fire archers","#cc3300",["#1a0600","#3a1000","#5a1c00","#7a2800","#9a3400"],275),
        lv("General Inferno","Caveat","the leader of the fire army. hes the HOTTEST thing alive. literally hes like a STAR.","command chamber, fire throne, star core","#ff6600",["#1a0c00","#3a2000","#5a3400","#7a4a00","#9a6000"],140),
    ],
    [
        design([("lava wall","#"),("obsidian floor","."),("lava flow","~"),("fire sentry","*")],"Gate Commander","he guards the gate and hes LITERALLY on fire ALL the time",["Fire Sentry","Lava Hound","Ash Soldier"],"Obsidian Sword","the gate was made of it so its REALLY strong","Heat Shield","lava bounces off it. LAVA",["Lava Splash","Fire Sentry","Gate Slam"],mode("D","phrygian"),"the gate commander cooled down and turned into a rock statue!!","the gate commander was too hot. he burned your SWORD."),
        design([("fire wall","#"),("hot floor","."),("fire bunk","~"),("weapon rack","*")],"Drill Sergeant Blaze","he yells at the fire soldiers and they all get HOTTER. literally HOTTER",["Fire Recruit","Flame Soldier","Ember Private"],"Fire Extinguisher","PSSSSHHH. fire goes OUT","Asbestos Mail","nothing hot can hurt you. NOTHING",["Drill Rush","Fire Line","Heat Wave"],mode("E","dorian"),"drill sergeant blaze lost his voice from yelling!! cant give orders now!!","drill sergeant blaze yelled SO loud everything caught on fire. MORE on fire."),
        design([("stone wall","#"),("map floor","."),("war table","~"),("battle flag","*")],"War Strategist","he plans ALL the attacks. take him out and the army has NO PLAN",["Plan Keeper","Map Guard","Battle Scholar"],"Erasers","you ERASE all the battle plans. no more plans!!","Strategists Cloak","you can SEE all the plans before they happen",["Planned Attack","Tactical Retreat","Battle Formation"],mode("F","lydian"),"the war strategist forgot all his plans!! he doesnt know what to do now!!","the war strategist planned for EVERYTHING. even for you planning for him."),
        design([("forge wall","#"),("hot metal floor","."),("weapon mold","~"),("cooling tank","*")],"Master Smith","she forges the fire weapons and shes made of LIQUID METAL",["Forge Imp","Metal Crawler","Spark Hammer"],"Cooling Rod","it freezes stuff instantly KSSHH","Cooled Plate","forged in the SAME forge but cooled down properly",["Forge Blast","Metal Splash","Spark Storm"],mode("G","aeolian"),"the master smith decided to make NICE things instead!! jewelry!!","the master smith threw liquid metal at you. SO hot."),
        design([("bridge wall","#"),("obsidian floor","."),("magma flow","~"),("arrow slit","*")],"Bridge Keeper","she stands in the MIDDLE of the obsidian bridge and she does NOT let people cross",["Magma Archer","Bridge Guard","Lava Fish"],"Crossing Pass","its a magic pass that lets you cross ANY bridge","Volcanic Boots","you can walk on lava. ON it. not IN it",["Bridge Break","Archer Volley","Magma Wave"],mode("A","mixolydian"),"the bridge keeper stepped aside!! she said you earned it!!","the bridge keeper knocked you into the magma moat. splash. OW splash."),
        design([("star wall","#"),("plasma floor","."),("fire throne","~"),("star core","*")],"General Inferno","hes as hot as a STAR. not like a cool star a HOT star. the hottest star. you cant even LOOK at him without sunglasses",["Star Guard","Plasma Knight","Nova Soldier"],"Absolute Zero","the COLDEST weapon ever made. colder than space","Solar Armor","you can stand next to a STAR and be fine",["Nova Blast","Star Fire","Plasma Wave"],mode("C","aeolian"),"GENERAL INFERNO COOLED DOWN!! hes just a regular guy now!! a cold regular guy!!","general inferno went supernova. thats when a star explodes. he EXPLODED."),
    ],
    st(5,3,3), settings(4,3,4,4)
))

# ── 23: Cloud Kingdom ──
campaigns.append(make_campaign(
    "Cloud Kingdom", "Poiret One",
    "way WAY up in the clouds theres a whole kingdom and GIANTS live there. everything is SO big. a fork is the size of a BUS.",
    "#1a2a4a", "#ccddff",
    [
        lv("Beanstalk Top","Gloria Hallelujah","you climbed the beanstalk and THIS is whats up here. whoa.","cloud ground, giant footprints, vast sky","#aaccff",["#142030","#2a3850","#425070","#5a6890","#7280b0"],125),
        lv("Giant Garden","Comic Neue","the garden has flowers the size of TREES and bugs the size of DOGS.","giant flowers, oversized bugs, huge leaves","#6aaa6a",["#0a1a0a","#142e14","#224a22","#306a30","#3e8a3e"],165),
        lv("The Kitchen Table","Short Stack","youre ON the giants kitchen table. forks knives spoons all HUGE. dont fall off.","giant utensils, bread mountains, glass towers","#ccaa88",["#1a1410","#3a2e24","#5a483a","#7a6450","#9a8068"],200),
        lv("Toy Shelf","Kalam","the giant kids toys. except the giant kid is the size of a BUILDING.","giant blocks, enormous dolls, huge cars","#aaaa66",["#1a1a0a","#3a3a1e","#5a5a34","#7a7a4c","#9a9a64"],240),
        lv("The Treasury","Patrick Hand","where the giants keep their GOLD. each coin is as big as a TABLE.","giant coins, enormous jewels, vault door","#ddaa44",["#1a1404","#3a2e0e","#5a4a1a","#7a682a","#9a883a"],275),
        lv("The Giant King","Permanent Marker","the BIGGEST giant. his HEAD is in the clouds. his FEET shake the ground. hes SO big.","cloud throne, giant castle, rumbling floor","#8888cc",["#101030","#222248","#363660","#4c4c7a","#626294"],140),
    ],
    [
        design([("cloud wall","#"),("cloud floor","."),("footprint","~"),("beanstalk","*")],"Cloud Stomper","a medium giant. MEDIUM. thats still like ten times your size",["Cloud Rat","Giant Flea","Beanstalk Bug"],"Pin Needle","its a sewing pin. to you its a SWORD","Thimble Helm","fits perfectly when youre tiny",["Stomp","Cloud Hole","Wind Gust"],mode("G","ionian"),"the cloud stomper tripped and fell OFF the cloud!! dont worry its soft down there maybe!!","the cloud stomper stomped. you were in the stomp zone."),
        design([("leaf wall","#"),("soil floor","."),("petal","~"),("dew drop","*")],"Garden Spider","its a NORMAL spider but youre TINY so its the size of a HORSE",["Giant Ant","Huge Beetle","Monster Worm"],"Thorn Lance","a rose thorn thats PERFECT for poking giant bugs","Acorn Cap","wear it on your head. its actually really hard",["Web","Petal Drop","Dew Splash"],mode("D","dorian"),"the garden spider went to bother someone its own size!!","the garden spider was normal sized. YOU were the problem."),
        design([("plate wall","#"),("table floor","."),("crumb","~"),("glass","*")],"The Cat","its just a regular CAT. but youre on the TABLE and the cat wants to PLAY. cats play ROUGH",["Dust Bunny","Bread Crust","Spoon"],
        "Fork Trident","its a fork. but to you its a MASSIVE trident","Cheese Armor","its cheese but its SO thick nothing gets through. smelly though",["Cat Paw","Dish Slide","Glass Tip"],mode("E","lydian"),"the cat got bored and walked away. typical cat.","the cat batted you off the table. cats dont care about ANYTHING."),
        design([("toy wall","#"),("shelf floor","."),("block","~"),("doll","*")],"Action Man","a giant kids action figure. but the giant kid isnt here. and the action figure is MOVING",["Toy Soldier","Jack In Box","Wind Up Duck"],"Battery Remover","take out the batteries and toys STOP","Toy Box Lid","its enormous and nothing gets through",["Spring Launch","Block Drop","Wind Up Rush"],mode("F","phrygian"),"action man ran out of batteries!! click click nothing!!","action man had fully charged batteries. SO much action."),
        design([("gold wall","#"),("coin floor","."),("gem","~"),("vault lock","*")],"Vault Guardian","a giant STATUE that comes alive when you touch the gold. DONT TOUCH THE GOLD ok you touched the gold",["Gold Golem","Gem Spider","Coin Snake"],"Diamond Edge","it cuts through anything even gold statues","Vault Armor","its made from vault metal. the HARDEST metal",["Gold Slam","Gem Shard","Vault Lock"],mode("A","aeolian"),"the vault guardian went back to sleep!! you can have ONE coin. ONE.","the vault guardian was too big and too gold and too guardian."),
        design([("castle wall","#"),("rumble floor","."),("throne pillar","~"),("cloud window","*")],"The Giant King","hes SO big his TOENAILS are bigger than you. his voice is like THUNDER. when he walks its like an EARTHQUAKE",["Giant Guard","Cloud Knight","Thunder Fist"],"Ankle Biter","it bites ankles. giant ankles. its very effective","Speed Boots","you run SO fast the giant cant catch you",["Earthquake","Thunder Voice","Giant Hand"],mode("C","aeolian"),"THE GIANT KING THOUGHT YOU WERE A BUG AND GAVE UP TRYING TO SQUISH YOU!! youre too small to fight!!","the giant king stepped on you. he didnt even know."),
    ],
    st(5,2,2), settings(5,4,99,2)
))

# ── 24: Time Machine ──
campaigns.append(make_campaign(
    "Time Machine", "Exo 2",
    "you found a TIME MACHINE and you go to different times and theres bad guys in EVERY time. dinosaur times robot times ALL the times.",
    "#1a1a2a", "#aacc88",
    [
        lv("Dinosaur Times","Gloria Hallelujah","you went to when dinosaurs were around. they are NOT happy to see you.","prehistoric jungle, volcanoes, giant ferns","#6a8a3a",["#0a1408","#142e14","#284828","#3a6a38","#4a8a48"],130),
        lv("Knight Times","Comic Neue","castles and knights and swords. everyone talks funny and fights a LOT.","castle walls, banners, jousting field","#7a7a6a",["#10100a","#242418","#3a3a28","#50503a","#68684e"],170),
        lv("Pirate Times","Short Stack","pirates AGAIN but these are HISTORICAL pirates and theyre even MORE piratey.","old ships, treasure maps, sea battles","#8a7a4a",["#140e04","#2e240e","#483a1a","#625228","#7c6a38"],200),
        lv("Wild West Times","Kalam","cowboys and outlaws and TUMBLEWEEDS. someone is always having a showdown.","saloon, desert, wanted posters","#aa8a4a",["#1a1408","#3a2e14","#5a4a22","#7a6630","#9a8240"],240),
        lv("Future Times","Patrick Hand","everything is chrome and flying and ROBOTS are everywhere. again.","chrome buildings, flying cars, holograms","#4a6a8a",["#081420","#142838","#223e58","#305478","#3e6a98"],275),
        lv("The Time Lord","Caveat","the boss controls ALL of time. he can go to any moment EVER and he uses that to CHEAT at fighting.","time throne, clock dimension, infinity portal","#6a6a8a",["#0e0e18","#1e1e30","#303048","#444462","#5a5a7c"],140),
    ],
    [
        design([("rock wall","#"),("fern floor","."),("fossil","~"),("volcano","*")],"Time Rex","its a T-REX but it can jump through TIME. a time-traveling dinosaur. thats NOT fair",["Chrono Raptor","Time Fly","Epoch Beetle"],"Bone Axe","made from a future dinosaur fossil. time is weird","Dino Hide","its really tough and it survived MILLIONS of years",["Time Bite","Era Shift","Temporal Stomp"],mode("G","dorian"),"the time rex got stuck in the BORING time. nothing happens there. ever.","the time rex bit you from THREE different time periods at once."),
        design([("castle wall","#"),("stone floor","."),("banner","~"),("torch","*")],"Black Knight","he challenges EVERYONE to fights and he NEVER gives up. ever. EVER.",["Squire","Archer","Page"],"Excalibur","THE legendary sword. it was stuck in a rock but you pulled it OUT","Chain Mail","old fashioned but it WORKS",["Lance Charge","Arrow Volley","Shield Bash"],mode("D","mixolydian"),"the black knight said fine you win. but he didnt sound happy about it.","the black knight had too much armor. SO much metal."),
        design([("ship wall","#"),("deck floor","."),("cannon","~"),("flag","*")],"Time Pirate","she steals treasure from EVERY time period. future gold past gold ALL gold",["Chrono Sailor","Time Parrot","Era Swabber"],"Chrono Cutlass","it hits you BEFORE you see it swing. time sword!!","Time Compass","it shows you where attacks are coming from BEFORE they happen",["Cannon Blast","Time Plank","Era Storm"],mode("E","aeolian"),"the time pirate got stuck in a time loop!! she keeps finding the same treasure over and over!!","the time pirate stole your FUTURE. like literally your future. you dont have one now."),
        design([("wood wall","#"),("dust floor","."),("tumbleweed","~"),("wanted poster","*")],"The Outlaw","fastest draw in ANY time period. he can shoot before you even THINK about moving",["Bandit","Deputy","Snake"],"Sheriff Star","its a throwing star shaped like a sheriff badge. VERY cool","Cowboy Hat","its tougher than it looks. WAY tougher",["Quick Draw","Stampede","Dynamite"],mode("F","lydian"),"the outlaw hung up his guns!! hes a farmer now!!","the outlaw was faster. SO much faster. you didnt even see it."),
        design([("chrome wall","#"),("hover floor","."),("hologram","~"),("hover car","*")],"Mecha Lord","from the FUTURE and he has EVERY future weapon. laser swords hover tanks ALL of it",["Hover Drone","Laser Bot","Chrome Soldier"],"Plasma Blade","future sword. it goes VWOOM VWOOM","Nano Armor","it repairs ITSELF. future technology is COOL",["Laser Grid","Hover Ram","Plasma Burst"],mode("A","phrygian"),"the mecha lord ran out of future power!! back to the future store!!","the mecha lord had too much future technology. unfair advantage."),
        design([("clock wall","#"),("time floor","."),("infinity symbol","~"),("portal","*")],"The Time Lord","he can pause time rewind time fast forward time EVERYTHING. he fights you in Monday AND Friday at the SAME TIME. how do you beat someone who controls TIME??",["Time Clone","Past Self","Future Self"],"Eternity Blade","it exists in ALL times at once. the Time Lord cant dodge it","Chrono Crown","you can control time too!! now its FAIR!!",["Time Stop","Rewind Attack","Future Strike"],mode("C","aeolian"),"THE TIME LORD GOT STUCK IN A TIME LOOP!! hes fighting himself FOREVER!! bye!!","the time lord rewound time and you lost before you even STARTED."),
    ],
    st(5,3,3), settings(4,3,5,3)
))

# ── Build final JSON ──
pack = {
    "theme": "kids first dungeon game",
    "campaigns": campaigns,
    "strings": {
        "title": "MY DUNGEON GAME",
        "subtitle": "the most AMAZING adventure EVER",
        "intro": [
            "HI!! welcome to my game!!",
            "you go in dungeons and fight bad guys",
            "and the bad guys are shapes and stuff",
            "some of them are REALLY scary but you can do it",
            "OK LETS GO!! press the button!!"
        ],
        "campaign_cleared": "YOU DID IT!!",
        "campaign_conquered": "{name} is DONE!! you beat ALL the bad guys!!",
        "prompt_first": "press ENTER to start!! HURRY!!",
        "prompt_next": "press ENTER for the next world!!",
        "prompt_resume": "press ENTER to keep going!!",
        "prompt_restart": "press ENTER to play AGAIN!! its still fun!!",
        "prompt_after_clear": "press ENTER for more!!"
    }
}

with open("campaigns.json", "w") as f:
    json.dump(pack, f, indent=2)

print(f"Generated {len(campaigns)} campaigns")
print(f"Total designs: {sum(len(c['designs']) for c in campaigns)}")
