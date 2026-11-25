"""Build the synthetic instance transport fixture with fontTools 4.66.0."""

from pathlib import Path
from tempfile import TemporaryDirectory

from fontTools.designspaceLib import AxisDescriptor, DesignSpaceDocument, SourceDescriptor
from fontTools.feaLib.builder import addOpenTypeFeaturesFromString
from fontTools.fontBuilder import FontBuilder
from fontTools.pens.ttGlyphPen import TTGlyphPen
from fontTools.ttLib import TTCollection, TTFont
from fontTools.varLib import build
from fontTools.varLib.instancer import instantiateVariableFont
from fontTools.varLib.featureVars import addFeatureVariations


AXES = (
    ("Weight", "wght", 100, 100, 900),
    ("Optical Size", "opsz", 8, 12, 72),
    ("Custom", "CSTM", 0, 0, 1),
    ("Width", "wdth", 75, 100, 125),
    ("Italic", "ital", 0, 0, 1),
    ("Slant", "slnt", -12, 0, 0),
)


def rectangle(width, height):
    pen = TTGlyphPen(None)
    pen.moveTo((0, 0))
    pen.lineTo((0, height))
    pen.lineTo((width, height))
    pen.lineTo((width, 0))
    pen.closePath()
    return pen.glyph()


def master(path, location):
    weight = (location["Weight"] - 100) / 8
    optical = location["Optical Size"] - 12
    custom = location["Custom"] * 60
    width = location["Width"] - 100
    italic = location["Italic"] * 15
    slant = -location["Slant"]
    delta = round(weight + optical + custom + width + italic + slant)
    ascender = 800 + round(weight)
    underline = -90 - round(weight / 2)
    advances = {
        ".notdef": 500,
        "A": 420 + delta,
        "V": 430 + delta,
        "A_V": 720 + delta,
        "X": 500,
        "Z": 600,
        "B": 420,
        "C": 420,
        "D": 420,
        "W": 500,
        "alef_hebrew": 500,
        "bet": 500,
        "gimel": 500,
        "lam": 440,
        "alef": 300,
        "lam.init": 440,
        "alef.fina": 300,
        "lam_alef": 620,
        "space": 250,
        "f": 380,
        "i": 280,
        "f_i": 580,
        "ka": 500,
        "matra_i": 200,
        "matra_i.pre": 220,
    }

    font = FontBuilder(1000, isTTF=True)
    font.setupGlyphOrder(list(advances))
    font.setupCharacterMap({
        **{ord(name): name for name in "ABCDVWXZfi"},
        0x0020: "space",
        0x05D0: "alef_hebrew",
        0x05D1: "bet",
        0x05D2: "gimel",
        0x0644: "lam",
        0x0627: "alef",
        0x0915: "ka",
        0x093F: "matra_i",
    })
    font.setupGlyf({
        name: TTGlyphPen(None).glyph() if name == "space" else rectangle(advance - 40, 680)
        for name, advance in advances.items()
    })
    font.setupHorizontalMetrics({name: (advance, 0) for name, advance in advances.items()})
    font.setupHorizontalHeader(ascent=ascender, descent=-200)
    font.setupNameTable(
        {
            "familyName": "Merman Instance Test",
            "styleName": "Regular",
            "psName": "MermanInstanceTest-Regular",
        }
    )
    font.setupOS2(
        version=4,
        fsSelection=1 << 7,
        sTypoAscender=ascender,
        sTypoDescender=-200,
        usWinAscent=950,
        usWinDescent=200,
    )
    font.setupPost(underlinePosition=underline, underlineThickness=50 + round(weight / 2))
    font.setupCOLR({"Z": [("A", 0)]}, version=0)
    font.setupCPAL([[(0.2, 0.4, 0.8, 1.0)]])
    addOpenTypeFeaturesFromString(
        font.font,
        """
        languagesystem DFLT dflt;
        languagesystem arab dflt;
        languagesystem dev2 dflt;
        feature liga { sub A V by A_V; sub f i by f_i; } liga;
        feature init { script arab; sub lam by lam.init; } init;
        feature fina { script arab; sub alef by alef.fina; } fina;
        feature rlig { script arab; sub lam.init alef.fina by lam_alef; } rlig;
        feature pres { script dev2; sub matra_i by matra_i.pre; } pres;
        """,
    )
    font.font["head"].created = font.font["head"].modified = 3660681600
    font.font.recalcTimestamp = False
    font.save(path)


def main():
    designspace = DesignSpaceDocument()
    defaults = {name: default for name, _, _, default, _ in AXES}
    locations = [defaults]
    for name, tag, minimum, default, maximum in AXES:
        designspace.addAxis(
            AxisDescriptor(name=name, tag=tag, minimum=minimum, default=default, maximum=maximum)
        )
        locations.extend(defaults | {name: value} for value in (minimum, maximum) if value != default)

    with TemporaryDirectory() as temp:
        for i, location in enumerate(locations):
            path = Path(temp) / f"master-{i}.ttf"
            master(path, location)
            designspace.addSource(SourceDescriptor(path=str(path), location=location))

        variable, _, _ = build(designspace)
        # Make these characters require fallback at the default weight.
        missing = dict.fromkeys(("A", "V", "lam", "alef", "ka", "matra_i"), ".notdef")
        addFeatureVariations(variable, [([{"wght": (0, 0.1)}], missing)])
        variable.recalcTimestamp = False
        collection = TTCollection()
        collection.fonts = [TTFont(Path(temp) / "master-0.ttf", recalcTimestamp=False), variable]
        collection.save(Path(__file__).with_name("instance-transport.ttc"))

        # A physical face whose only style axis is slnt.
        pins = {tag: default for _, tag, _, default, _ in AXES if tag != "slnt"}
        pins["wght"] = 400
        slanted = instantiateVariableFont(variable, pins)
        slanted.recalcTimestamp = False
        slanted.save(Path(__file__).with_name("slant-only.ttf"))


if __name__ == "__main__":
    main()
