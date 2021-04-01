#!/usr/bin/pytest-3
# import pytest
import postprocess

# These tests assume the example file postprocess.py
# It's recommended to add/modify tests here for every addition or modification there.

sep = "|"


def put_out(input):
    # this code sounds weird :-)
    return sep.join(postprocess.postprocess(input.split(sep), sep))


# keep in mind that these are run _after_ the regex magic has taken place

def test_nochange():
    # basic test for stuff that doesn't match the input filter
    # this should give us the unchanged list we provided
    input = "Name|SubLocation|Location|ProvinceState|Country|1.11.2001|Creator"
    assert input == put_out(input)


def test_glob1():
    # test for global replacements (Zurich should be replaced by Zürich)
    input = "Name|SubLocation|Location|Zurich|Country|1.11.2001|Creator"
    assert put_out(input) == "Name|SubLocation|Location|Zürich|Country|1.11.2001|Creator"


def test_glob2():
    # test for global replacements (Zurich should be replaced by Zürich)
    input = "Name|SubLocation|Zurich|Zurich|Country|1.11.2001|Creator"
    assert put_out(input) == "Name|SubLocation|Zürich|Zürich|Country|1.11.2001|Creator"


def test_glob3():
    # test for global replacements (Zurich should be replaced by Zürich)
    input = "Zurich|SubLocation|Zurich|Zurich|Country|1.11.2001|Creator"
    assert put_out(input) == "Zürich|SubLocation|Zürich|Zürich|Country|1.11.2001|Creator"


def test_glob4():
    # test for global replacements (Zurich should be replaced by Zürich, ' Township', ' City', ' Province'
    # should be removed)
    input = "Zurich Township|SubLocation Township|Location City|Zurich Province|Country|1.11.2001|Creator"
    assert put_out(input) == "Zürich|SubLocation|Location|Zürich|Country|1.11.2001|Creator"


# we have to make up a test for every filtered item of input
# now, we have Dia, Südkorea, Mark, Marokko, Schweiz

# Südkorea tests
def test_slide1():
    # we remove all info except creator from images that are duplicates of slides
    input = "From Slide|SubLocation|Location|ProvinceState|Country|1.11.2001|Creator"
    assert put_out(input) == "From Slide|Creator"


# Südkorea tests
def test_skorea1():
    # we generally want to omit the province in S Korea
    # so the S Korea function should return Name|SubLocation|Location|South Korea|1.11.2001|Creator
    input = "Name|SubLocation|Location|ProvinceState|South Korea|1.11.2001|Creator"
    assert put_out(input) == "Name|SubLocation|Location|South Korea|1.11.2001|Creator"


def test_skorea2():
    # except when ProvinceState is Busan, then it should return
    # Name|Busan|South Korea|1.11.2001|Creator
    for province in ["Busan", "Jeju", "Seoul"]:
        input = "Name|SubLocation|Location|" + province + "|South Korea|1.11.2001|Creator"
        assert put_out(input) == "Name|SubLocation|Location|" + province + "|South Korea|1.11.2001|Creator"


def test_skorea3():
    # tests landmark adding
    input = "|Pungcheon|Location|ProvinceState|South Korea|1.11.2001|Creator"
    assert put_out(input) == "Hahoe/Byeongsanseowon|Location|South Korea|1.11.2001|Creator"


# Morocco test
def test_morocco():
    # we omit the province
    # the Morocco function should return Name|SubLocation|Location|Marokko|1.11.2001|Creator
    input = "Name|SubLocation|Location|ProvinceState|Morocco|1.11.2001|Creator"
    assert put_out(input) == "Name|SubLocation|Location|Morocco|1.11.2001|Creator"


def test_morocco2():
    # tests landmark adding
    input = "|SubLocation|M'Semrir|ProvinceState|Morocco|1.11.2001|Creator"
    assert put_out(input) == "Gorges du Dades|SubLocation|M'Semrir|Morocco|1.11.2001|Creator"


def test_morocco3():
    # Marrakesh should keep the province
    input = "|SubLocation|Marrakech|Marrakech|Morocco|1.11.2001|Creator"
    assert put_out(input) == "|SubLocation|Marrakech|Marrakech|Morocco|1.11.2001|Creator"


# Schweiz tests
def test_ch1():
    # generally, we do not change anything...
    # the Switzerland function should return "Name|SubLocation|Location|ProvinceState|Switzerland|1.11.2001|Creator" unchanged
    input = "Name|SubLocation|Location|ProvinceState|Switzerland|1.11.2001|Creator"
    assert put_out(input) == input


def test_ch2():
    # ...except when input is "Name|SubLocation|Location|Kanton Zürich|Schweiz|1.11.2001|Creator"
    # then we want to see "Name|SubLocation|Location ZH|1.11.2001|Creator"
    for canton in postprocess.cantons.keys():
        input = "Name|SubLocation|Location|Kanton " + canton + "|Switzerland|1.11.2001|Creator"
        assert put_out(input) == "Name|SubLocation|Location " + postprocess.cantons.get(canton) + "|1.11.2001|Creator"


def test_ch3():
    # same as 2, but with glob replacement ('Zurich Province'-> 'Zürich' and then as in #2 above)
    input = "Name|SubLocation|Location|Kanton Zurich Province|Switzerland|1.11.2001|Creator"
    assert put_out(input) == "Name|SubLocation|Location ZH|1.11.2001|Creator"


def test_ch4():
    # ...except when input is "Name|SubLocation|Zürich|Kanton Zürich|Schweiz|1.11.2001|Creator"
    # when the canton's name is in the city name
    # then we want to see "Name|SubLocation|Zürich|1.11.2001|Creator"
    cities = ['Zürich', 'Basel', 'St. Gallen']
    for city in cities:
        for canton in postprocess.cantons.keys():
            if city in canton:
                input = "Name|SubLocation|" + city + "|Kanton " + canton + "|Switzerland|1.11.2001|Creator"
                assert put_out(input) == "Name|SubLocation|" + city + "|1.11.2001|Creator"


# date timeline tests: they assume the matching timespans in postprocess.py
def test_timeline1():
    # this should return USA as country
    input = "Name|SubLocation|Location|ProvinceState||08-13-2012|Creator"
    assert put_out(input) == "Name|SubLocation|Location|ProvinceState|USA|08-13-2012|Creator"


def test_timeline2():
    # this should return USA as country, NY as state, 'In teh Catskills' as sublocation, but leave Location untouched
    input = "||Location|||08-20-2012|Creator"
    assert put_out(input) == "|In the Catskills|Location|NY|USA|08-20-2012|Creator"


def test_timeline3():
    # incorrect/no date
    # this should return unchanged
    input = "Name|SubLocation|Location|ProvinceState||Date|Creator"
    assert put_out(input) == input


def test_timeline4():
    # too few items
    # this should return unchanged
    input = "Name|Date"
    assert put_out(input) == input


def test_timeline5():
    # minimal items, tests that we do not go beyond left border
    # this will not work when there's more in timespan than the country only
    # (practically, this is not happening, because we would feed it with more empty items...)
    # see #8 for this
    # this should return USA as country
    input = "|05-01-2019|Creator"
    assert put_out(input) == "USA|05-01-2019|Creator"


def test_timeline6():
    # country name already present
    # this should return unchanged
    input = "Name|SubLocation|Location|ProvinceState|USA|05-01-2019|Creator"
    assert put_out(input) == input


def test_timeline7():
    # compare this to #5 to see the difference between missing and empty input
    input = "|||||08-20-2012|Creator"
    assert put_out(input) == "|In the Catskills||NY|USA|08-20-2012|Creator"


def test_timeline8():
    # this should return no values except 'From Slide|Creator'
    input = "|SubLocation|Location|ProvinceState|Country|10-16-2020|Creator"
    assert put_out(input) == "From Slide|Creator"

# def test_timeline9():
#    with pytest.raises(ValueError, match=r"Too many items in timespan:*"):
# too many timespan entries test
# add the line below to the timespans before running this
# '19141008': {'19141008': {'USA': {'PA': {'Philadelphia': {'30th Street Station':{ 'Something':{ 'This here is too much': None}}}}}}},
#        input = "||||8.10.1914|Creator"
#        print(put_out(input))
