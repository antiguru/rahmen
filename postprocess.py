import re

# python code that takes a list of the metadata tags, after they have
# been processed using the regex definitions,
# and processes them accordingly. It is currently required that this will return
# a list of items (strings).
#
# This File assumes some regex processing being done in the configuration file (see rahmen.toml example)
#
# We used a positional approach here. We know how many tags we configured to be shown in the configuration file;
# thus we can move back or forth from any given item and find another that way.
# Let's say we have defined the field structure Info, Quarter, District_or_City, ProvinceState, Country, Date, Creator,
# and we have looked for 'USA' and got the index for it. Assuming that 'USA' is in the 'country' field
# (you'll have to take care that you choose your search terms carefully so that false positives are ruled out),
# the structure is then Info, Quarter, District_or_City, ProvinceState, USA, Date, Creator
# and the offsets:      ^^^⁻4,^^^-3,   ^^^-2,            ^^^-1          ^^^  ^^^+1 ^^^+2
# so the 'info' field would be at index-4, the 'creator' field at index+2
#
# The output will be unconditionally cleaned of empties and uniquified unless you return a list of just one item
# (see the example at the end)
#
# this holds the item positions we want to drop
# dropping cannot be done ad hoc because it would shift the positions
delx = []

def append_to_delete(ix):
    if ix not in delx:
        delx.append(ix)

def modify(items, ix_to_check, val_to_check, ix_to_modify, val_to_modify, ix_to_delete=None):
    # modify an item in items if it is not set depending on s/th matching another item,
    # optionally mark item for deletion
    #
    # if this item matches the control value, and
    if items[ix_to_check] == val_to_check:
        # if the item to be modified is not set,
        if not items[ix_to_modify]:
            # modify it
            items[ix_to_modify] = val_to_modify
        # optionally mark item[ix] for deletion
        if ix_to_delete:
            append_to_delete(ix_to_delete)


def pp_s_korea(items, it, ix):
    # look for the item before the country ('South Korea'), it's ProvinceState
    # the structure is then Info, Quarter, District_or_City, ProvinceState, South Korea, Date, Creator
    # the offsets:          ^^^⁻4,^^^-3,   ^^^-2,            ^^^-1          ^^^we start here
    # the following assumes that the province suffix '-do' has already been regexed away
    #
    # ...in the big cities  and in Jeju, the name of the province is the well-known name, so keep it
    if items[ix - 1] not in ["Seoul", "Jeju", "Busan"]:
        # ...otherwise drop the province
        append_to_delete(ix - 1)
    # cut away city quarter overkill
    quarter_parts = items[ix - 3].split(' ')
    if len(quarter_parts) > 1:
        items[ix - 3] = quarter_parts[0]
    # set some landmark names from the district quarter
    # TODO we could move this to a dict
    modify(items, ix - 3, 'Sanga', ix - 4, 'Woryeonggyo Bridge', ix - 3)
    modify(items, ix - 3, 'Pungcheon', ix - 4, 'Hahoe/Byeongsanseowon', ix - 3)
    modify(items, ix - 3, 'Jinhyeon', ix - 4, 'Bulguksa/Seokguram', ix - 3)
    modify(items, ix - 3, 'Cheongnyong', ix - 4, 'Beomeosa', ix - 3)
    return items


def pp_morocco(items, it, ix):
    # drop the province, except when it's Marrake([s|c]h)
    if not 'Marrakech' in items[ix - 1]:
        append_to_delete(ix - 1)
    # set some landmark names from the city
    modify(items, ix - 2, "M'Semrir", ix - 4, 'Gorges du Dades')
    modify(items, ix - 2, "Zerkten", ix - 4, "Tizi n'Tichka")
    modify(items, ix - 2, "Mezguita", ix - 4, "Tamnougalt")
    modify(items, ix - 2, "Ikniouen", ix - 4, "Jbel Saghro")

    return items


# cantons and abbreviations for them, to be extended
cantons = {'Zürich': 'ZH', 'Basel-Stadt': 'BS', 'St. Gallen': 'SG'}


# Someplace, Canton of Zürich, => Someplace ZH, unless Someplace in 'Canton of Zürich'
def pp_ch_cantons(items, ix):
    ct = ''
    for canton in cantons.keys():
        # input fields
        input_canton = items[ix - 1]
        city = items[ix - 2]
        # if the dict term is in the input input_canton
        if canton in input_canton:
            # and if the city name is not port of the dict canton
            if city not in canton:
                # append the canton's abbreviation
                ct = ' ' + cantons.get(canton)
            # mark city and country field for deletion
            append_to_delete(ix)
            append_to_delete(ix - 2)
            # update input_canton field with city + abbreviation (or '')
            items[ix - 1] = city + ct
    return items

# This defines timespans per country (province/state,(city, (...)).
# These are tuples (immutable lists) of values.
# ('Start date','End date','Country','ProvinceState', 'City','Quarter','Info')
# This matches the metatags that are defined in the config file, but the order is reversed here.
# This example assumes that the config file metatags amount to
# Name, Sublocation, Location, ProvinceState, Country, Date, Creator
# We match between and including start and end dates and proceed left from there: Country, ProvinceState, etc.
# Date format here is YYYYMMDD (unlike in the display and config file).
# This way, un-geotagged images will be associated with the country or places you visited
# To skip items (leave them untouched), insert an empty string.
# Existing entries will not be overwritten.
# The start date has to be unique. Do not overlap end dates, when they do, the entry starting first wins.
timespans = (
    ('20120812', '20120814', 'USA', 'NY', 'New York'),
    ('20120815', '20120821', 'USA', 'NY', '', 'In the Catskills'),
    ('20131019', '20131019', 'USA', 'NV', 'Pyramid Lake'),
    ('20141019', '20141019', 'USA', '', 'Lake Tahoe'),
    ('20190420', '20190522', 'USA'),
    ('20200110', '20200122', 'Portugal'),
    ('20011101', '20011101', '', '', '', '', 'From Slide'),
)

# add information to image if the image data is inside a timespan
def pp_metadata_from_timespan(items):
    # we assume that 'date' is the item before the last (hence the i_start is set to 2 below,
    # lenght(items)-i_start pointing to that place) and that it's formatted d.m.yyyy
    # and that 'country' is the item before date
    # this has to be configured that way in the configuration file
    # no real error checking is being done here
    # this assumes the item list is configured like this:
    # ['info', 'sublocation', 'location', 'provincestate', 'country', 'date', 'creator']
    # we start at date, which is at len()-2                            ^^^-2
    # so our starting value is 2
    i_start = 2
    # which gives us the position of date
    i_pos = len(items) - i_start
    # we need at least country|date|creator, so, more items than just counted from or starting point
    if len(items) > i_start:
        # let's look at what's at the date position and try to
        # get the strings for day, month, year (input format m-d-yyyy)
        # this should give us M, D, YYYY @ 0, 1, 2
        i_date_list = items[i_pos].split('-')
        # without 3 items, it's not a correct date (this is only _very_ basic error checking)
        if len(i_date_list) == 3:
            # convert the date string to YYYYMMDD to make it sortable (add leading zeros if necessary)
            i_date = i_date_list[2] + i_date_list[0].zfill(2) + i_date_list[1].zfill(2)
            for timespan in timespans:
                # make timespan tuple iterable
                timespan_iter = iter(timespan)
                # get first tuple from the timespans
                # to compare with the item's date
                start_date = next(timespan_iter)
                if i_date >= start_date:
                    end_date = next(timespan_iter)
                    if i_date <= end_date:
                        # we have a hit on the timespan
                        # now, move the position to the item before the date (should be 'country')
                        i_pos = i_pos - 1
                        # don't wrap around the item list (this catches and ignores too many items in timespan tuple)
                        while i_pos >= 0:
                            # this try block catches the end of the timespan tuple because it will be left if there's
                            # no more 'next'
                            try:
                                # work through the timespan tuple:
                                # get the next new item from the timespan tuple...
                                new_item = next(timespan_iter)
                                # ...set the metadata item from the timespan item if it isn't set already
                                # (and is not empty)...
                                if not items[i_pos]:
                                    items[i_pos] = new_item
                                # ...move our item pointer to the previous item (we're going backward)
                                i_pos = i_pos - 1
                            except StopIteration:
                                # no more items in tuple, so we're done
                                break
    return (items)
# consume the rest of the timespan after country

# for slides we delete all info that may have been set from the camera's GPS
def pp_dia():
    for i in [1, 2, 3, 4, 5]:
        append_to_delete(i)


# global replacements: the dictionary has keys (to look up) and replacement values.
def pp_glob(items, glob_replacements):
    for i, it in enumerate(items):
        for key, value in glob_replacements.items():
            # update the working value to prevent regressions when multiple matches occur
            it = re.sub(key, value, it)
            items[i] = it
    return items


# value/replacement dictionary
# these will be replaced wherever they occur
# regular expressions [https://docs.python.org/3/library/re.html] are allowed
glob_replacements = {'Zurich': 'Zürich',
                     ' City': '',
                     ' Township': '',
                     ' District': '',
                     ' Province': '',
                     }


# main filter
def postprocess(items: [str], sep: str) -> str:
    outitems = []
    # clear the drop list
    delx.clear()
    # first, replace the global stuff
    items = pp_glob(items, glob_replacements)
    # get metadata from timespans
    # (if it was me who photographed)
    # if 'Hartmut' in items[len(items)-1]:
    items = pp_metadata_from_timespan(items)
    print(items)
    # now the specific filters
    for ix, it in enumerate(items):
        if 'From Slide' in it:
            pp_dia()
            outitems = items
        if it == "South Korea":
            outitems = pp_s_korea(items, it, ix)
        if it == "Morocco":
            outitems = pp_morocco(items, it, ix)
        if it == "Switzerland":
            outitems = pp_ch_cantons(items, ix)

    if not outitems:
        print("Status line unfiltered.")
    else:
        # only now, we remove the dropped items
        for x in sorted(delx, reverse=True):
            del outitems[x]
        print("Status line changed to:")
        print(outitems)
        items = outitems

    # if you return a list of items, the final processing will cause empties to be filtered out and
    # multiple items to be returned only once (unconditionally), and finally join them to a line using the
    # separator string.
    return items
    # if you return a list of only one item, the final processing step will see this as a non-empty, unique, single item
    # and will return it unmodified. This way, you can define the final processing step here. The example drops
    # empty items and joins them with the separator, but does not remove multiples.
    # return [sep.join(filter(lambda x: len(x) > 0, items))]


def export():
    # return callable to Rust code
    return postprocess
