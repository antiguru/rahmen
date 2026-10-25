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
    input = "From Slide|SubLocation|Location|ProvinceState|Country|11-01-2001|Creator"
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
    input = "|Pungcheon|Location|ProvinceState|South Korea|2.11.2001|Creator"
    assert put_out(input) == "Hahoe/Byeongsanseowon|Location|South Korea|2.11.2001|Creator"


# Morocco test
def test_morocco():
    # we omit the province
    # the Morocco function should return Name|SubLocation|Location|Marokko|2.11.2001|Creator
    input = "Name|SubLocation|Location|ProvinceState|Morocco|2.11.2001|Creator"
    assert put_out(input) == "Name|SubLocation|Location|Morocco|2.11.2001|Creator"


def test_morocco2():
    # tests landmark adding
    input = "|SubLocation|M'Semrir|ProvinceState|Morocco|2.11.2001|Creator"
    assert put_out(input) == "Gorges du Dades|SubLocation|M'Semrir|Morocco|2.11.2001|Creator"


def test_morocco3():
    # Marrakesh should keep the province
    input = "|SubLocation|Marrakech|Marrakech|Morocco|2.11.2001|Creator"
    assert put_out(input) == "|SubLocation|Marrakech|Marrakech|Morocco|2.11.2001|Creator"


# Schweiz tests
def test_ch1():
    # generally, we do not change anything...
    # the Switzerland function should return "Name|SubLocation|Location|ProvinceState|Switzerland|2.11.2001|Creator" unchanged
    input = "Name|SubLocation|Location|ProvinceState|Switzerland|2.11.2001|Creator"
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
    input = "|SubLocation|Location|ProvinceState|Country|11-01-2001|Creator"
    assert put_out(input) == "From Slide|Creator"

# def test_timeline9():
#    with pytest.raises(ValueError, match=r"Too many items in timespan:*"):
# too many timespan entries test
# add the line below to the timespans before running this
# '19141008': {'19141008': {'USA': {'PA': {'Philadelphia': {'30th Street Station':{ 'Something':{ 'This here is too much': None}}}}}}},
#        input = "||||8.10.1914|Creator"
#        print(put_out(input))


def test_timeline_date_formats():
    for date in ['2012-08-13', '13.08.2012', '08-13-2012']:
        assert put_out('|||||' + date + '|Creator') == '||New York|NY|USA|' + date + '|Creator'


def test_timeline_invalid_dates():
    for date in ['', '2012-02-30', '02-30-2012', '30.02.2012', '2012-13-01',
                 'not-a-date', '2012-08-13-extra', '2012-08-13 12:00']:
        original = '|||||' + date + '|Creator'
        assert put_out(original) == original


def test_timeline_overlap_earliest_start_wins():
    original = postprocess.timespans
    try:
        postprocess.timespans = (
            ('20120812', '20120815', 'Later', 'Later state'),
            ('20120810', '20120815', 'Earlier'),
        )
        assert put_out('|||||08-13-2012|Creator') == '||||Earlier|08-13-2012|Creator'
    finally:
        postprocess.timespans = original


def test_timeline_preserves_existing_metadata():
    assert put_out('Title|Quarter|City|State|Country|2012-08-13|Creator') == (
        'Title|Quarter|City|State|Country|2012-08-13|Creator')


def test_short_country_lists():
    for country in ['South Korea', 'Morocco', 'Switzerland']:
        for items in [[], [country], [country, 'Creator'], [country, 'Date', 'Creator'],
                      ['', country, 'Date', 'Creator'],
                      ['Quarter details', 'City', 'Province', country, 'Date', 'Creator']]:
            assert postprocess.postprocess(items.copy(), sep) == items


def test_country_names_outside_country_field():
    for country in ['South Korea', 'Morocco', 'Switzerland']:
        original = country + '|Quarter|City|Kanton Zürich|Country|Date|' + country
        assert put_out(original) == original


def test_swiss_missing_city():
    original = 'Title|Quarter||Kanton Zürich|Switzerland|Date|Creator'
    assert put_out(original) == original


def test_modify_deletes_zero_index():
    items = ['City', 'Control']
    deletions = set()
    postprocess.modify(items, 1, 'Control', 0, 'Replacement', 0, delx=deletions)
    assert items == ['City', 'Control']
    assert deletions == {0}


def test_short_slide_list():
    assert put_out('From Slide') == 'From Slide'


def test_processing_is_silent():
    import contextlib
    import io
    output = io.StringIO()
    with contextlib.redirect_stdout(output):
        assert postprocess.export()(['Title', 'Date', 'Creator'], sep) == ['Title', 'Date', 'Creator']
    assert output.getvalue() == ''


def test_slide_date_formats():
    for date in ['2001-11-01', '1.11.2001', '11-01-2001']:
        assert put_out('|Quarter|City|Province|Country|' + date + '|Creator') == 'From Slide|Creator'


def test_timeline_rejects_invalid_calendar_date_in_span():
    original = postprocess.timespans
    try:
        postprocess.timespans = (('20120201', '20120301', 'Country'),)
        for date in ['2012-02-30', '02-30-2012', '30.02.2012']:
            assert put_out('||' + date + '|Creator') == '||' + date + '|Creator'
        assert put_out('||2012-02-29|Creator') == '|Country|2012-02-29|Creator'
    finally:
        postprocess.timespans = original


def test_processing_deletions_are_isolated():
    original = postprocess.pp_s_korea
    nested_items = ['Title', 'Date', 'Creator']
    try:
        def nested_processing(*args):
            result = original(*args)
            assert postprocess.postprocess(nested_items.copy(), sep) == nested_items
            return result
        postprocess.pp_s_korea = nested_processing
        assert put_out('Title|Quarter|City|Province|South Korea|Date|Creator') == (
            'Title|Quarter|City|South Korea|Date|Creator')
        assert postprocess.postprocess(nested_items.copy(), sep) == nested_items
    finally:
        postprocess.pp_s_korea = original


def test_slide_marker_outside_title():
    for index in range(1, 7):
        items = ['Title', 'Quarter', 'City', 'Province', 'Country', 'Date', 'Creator']
        items[index] = 'From Slide archive'
        assert postprocess.postprocess(items.copy(), sep) == items
    assert put_out('Title From Slide archive|Quarter|City|Province|Country|Date|Creator') == (
        'Title From Slide archive|Creator')


def test_modify_does_not_wrap_negative_index():
    items = ['Control', '']
    deletions = set()
    postprocess.modify(items, 0, 'Control', -1, 'Replacement', 0, delx=deletions)
    assert items == ['Control', '']
    assert deletions == set()


def test_timeline_does_not_wrap_short_list():
    assert put_out('|2012-08-13|Creator') == 'USA|2012-08-13|Creator'
