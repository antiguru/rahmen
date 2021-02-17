#!/usr/bin/pytest-3
import postprocess

sep = "|"


def put_out(input):
    # this code sounds weird :-)
    return sep.join(postprocess.postprocess(input.split(sep), sep))


# keep in mind that these are run _after_ the regex magic has taken place

def test_nochange():
    # basic test for stuff that doesn't match the input filter
    # this should give us the unchanged list we provided
    input = "Name|SubLocation|Location|ProvinceState|Country|Date|Creator"
    assert input == put_out(input)


def test_glob1():
    # test for global replacements (Zurich should be replaced by Zürich)
    input = "Name|SubLocation|Location|Zurich|Country|Date|Creator"
    assert put_out(input) == "Name|SubLocation|Location|Zürich|Country|Date|Creator"


def test_glob2():
    # test for global replacements (Zurich should be replaced by Zürich)
    input = "Name|SubLocation|Zurich|Zurich|Country|Date|Creator"
    assert put_out(input) == "Name|SubLocation|Zürich|Zürich|Country|Date|Creator"


def test_glob3():
    # test for global replacements (Zurich should be replaced by Zürich)
    input = "Zurich|SubLocation|Zurich|Zurich|Country|Date|Creator"
    assert put_out(input) == "Zürich|SubLocation|Zürich|Zürich|Country|Date|Creator"


def test_glob4():
    # test for global replacements (Zurich should be replaced by Zürich, ' Township', ' City', ' Province'
    # should be removed)
    input = "Zurich Township|SubLocation Township|Location City|Zurich Province|Country|Date|Creator"
    assert put_out(input) == "Zürich|SubLocation|Location|Zürich|Country|Date|Creator"


# we have to make up a test for every filtered item of input
# now, we have Südkorea, Mark, Marokko, Schweiz

# Südkorea tests
def test_skorea1():
    # we generally want to omit the province in S Korea
    # so the S Korea function should return Name|Location|Südkorea|Date|Creator
    input = "Name|SubLocation|Location|ProvinceState|Südkorea|Date|Creator"
    assert put_out(input) == "Name|Location|Südkorea|Date|Creator"


def test_skorea2():
    # except when ProvinceState is Busan, then it should return
    # Name|Busan|Südkorea|Date|Creator
    input = "Name|SubLocation|Location|Busan|Südkorea|Date|Creator"
    assert put_out(input) == "Name|Busan|Südkorea|Date|Creator"


def test_skorea3():
    # except when ProvinceState is Seoul, then it should return
    # Name|Seoul|Südkorea|Date|Creator
    input = "Name|SubLocation|Location|Seoul|Südkorea|Date|Creator"
    assert put_out(input) == "Name|Seoul|Südkorea|Date|Creator"


def test_skorea4():
    # except when ProvinceState is Jeju, then it should return
    # Name|Location|Jeju|Südkorea|Date|Creator
    input = "Name|SubLocation|Location|Jeju|Südkorea|Date|Creator"
    assert put_out(input) == "Name|Location|Jeju|Südkorea|Date|Creator"


# Mark Brandenburg test
def test_mark():
    # the Mark function should return Name|Sublocation|Location (Mark)|Country|Date|Creator
    input = "Name|SubLocation|Location|Mark|Country|Date|Creator"
    assert put_out(input) == "Name|SubLocation|Location (Mark)|Country|Date|Creator"


# Marokko test
def test_morocco():
    # we omit the province
    # the Morocco function should return Name|SubLocation|Location|Marokko|Date|Creator
    input = "Name|SubLocation|Location|ProvinceState|Marokko|Date|Creator"
    assert put_out(input) == "Name|SubLocation|Location|Marokko|Date|Creator"


# Schweiz tests
def test_ch1():
    # generally, we do not change anything...
    # the Switzerland function should return "Name|SubLocation|Location|ProvinceState|Schweiz|Date|Creator" unchanged
    input = "Name|SubLocation|Location|ProvinceState|Schweiz|Date|Creator"
    assert put_out(input) == input


def test_ch2():
    # ...except when input is "Name|SubLocation|Location|Kanton Zürich|Schweiz|Date|Creator"
    # then we want to see "Name|SubLocation|Location ZH|Schweiz|Date|Creator"
    input = "Name|SubLocation|Location|Kanton Zürich|Schweiz|Date|Creator"
    assert put_out(input) == "Name|SubLocation|Location ZH|Schweiz|Date|Creator"

def test_ch3():
    # same as 2, but with glob replacement ('Zurich Province'-> 'Zürich' and then as in #2 above)
    input = "Name|SubLocation|Location|Kanton Zurich Province|Schweiz|Date|Creator"
    assert put_out(input) == "Name|SubLocation|Location ZH|Schweiz|Date|Creator"

def test_ch4():
    # ...except when input is "Name|SubLocation|Zürich|Kanton Zürich|Schweiz|Date|Creator"
    # when the canton's name is in the city name
    # then we want to see "Name|SubLocation|Zürich|Schweiz|Date|Creator"
    input = "Name|SubLocation|Zürich|Kanton Zürich|Schweiz|Date|Creator"
    assert put_out(input) == "Name|SubLocation|Zürich|Schweiz|Date|Creator"

def test_ch5():
    # ...except when input is "Name|SubLocation|Zürich|Kanton Zürich|Schweiz|Date|Creator"
    # when the canton's name is in the city name
    # then we want to see "Name|SubLocation|Zürich|Schweiz|Date|Creator"
    input = "Name|SubLocation|Basel|Kanton Basel-Stadt|Schweiz|Date|Creator"
    assert put_out(input) == "Name|SubLocation|Basel|Schweiz|Date|Creator"


