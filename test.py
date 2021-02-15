#!/usr/bin/pytest-3
import postprocess

sep = "|"


# keep in mind that these are run _after_ the regex magic has taken place

def test_nochange():
    # basic test for stuff that doesn't match the input filter
    # this should give us the unchanged list we provided
    input = "Name|SubLocation|Location|ProvinceState|Country|Date|Creator"
    output = postprocess.postprocess(input.split(sep), sep)
    assert input == output


# we have to make up a test for every filtered item of input
# now, we have Südkorea, Mark, Marokko, Schweiz

# Südkorea tests
def test_skorea1():
    # we generally want to omit the province in S Korea
    # so the S Korea function should return Name|Location|Südkorea|Date|Creator
    input = "Name|SubLocation|Location|ProvinceState|Südkorea|Date|Creator"
    output = postprocess.postprocess(input.split(sep), sep)
    assert output == "Name|Location|Südkorea|Date|Creator"


def test_skorea2():
    # except when ProvinceState is Busan, then it should return
    # Name|Busan|Südkorea|Date|Creator
    input = "Name|SubLocation|Location|Busan|Südkorea|Date|Creator"
    output = postprocess.postprocess(input.split(sep), sep)
    assert output == "Name|Busan|Südkorea|Date|Creator"


def test_skorea3():
    # except when ProvinceState is Seoul, then it should return
    # Name|Seoul|Südkorea|Date|Creator
    input = "Name|SubLocation|Location|Seoul|Südkorea|Date|Creator"
    output = postprocess.postprocess(input.split(sep), sep)
    assert output == "Name|Seoul|Südkorea|Date|Creator"


def test_skorea4():
    # except when ProvinceState is Jeju, then it should return
    # Name|Location|Jeju|Südkorea|Date|Creator
    input = "Name|SubLocation|Location|Jeju|Südkorea|Date|Creator"
    output = postprocess.postprocess(input.split(sep), sep)
    assert output == "Name|Location|Jeju|Südkorea|Date|Creator"


# Mark Brandenburg test
def test_mark():
    # the Mark function should return Name|Sublocation|Location (Mark)|Country|Date|Creator
    input = "Name|SubLocation|Location|Mark|Country|Date|Creator"
    output = postprocess.postprocess(input.split(sep), sep)
    assert output == "Name|SubLocation|Location (Mark)|Country|Date|Creator"


# Marokko test
def test_morocco():
    # we omit the province
    # the Morocco function should return Name|SubLocation|Location|Marokko|Date|Creator
    input = "Name|SubLocation|Location|ProvinceState|Marokko|Date|Creator"
    output = postprocess.postprocess(input.split(sep), sep)
    assert output == "Name|SubLocation|Location|Marokko|Date|Creator"


# Schweiz tests
def test_ch1():
    # generally, we do not change anything...
    # the Switzerland function should return "Name|SubLocation|Location|ProvinceState|Schweiz|Date|Creator" unchanged
    input = "Name|SubLocation|Location|ProvinceState|Schweiz|Date|Creator"
    output = postprocess.postprocess(input.split(sep), sep)
    assert output == input


def test_ch2():
    # ...except when input is "Name|SubLocation|Location|Kanton Zürich|Schweiz|Date|Creator"
    # then we want to see "Name|SubLocation|Location ZH|Schweiz|Date|Creator"
    input = "Name|SubLocation|Location|Kanton Zürich|Schweiz|Date|Creator"
    output = postprocess.postprocess(input.split(sep), sep)
    assert output == "Name|SubLocation|Location ZH|Schweiz|Date|Creator"
