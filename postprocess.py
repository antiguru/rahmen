import re
from datetime import datetime

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
# (the country is matched only at its configured position to avoid false positives),
# the structure is then Info, Quarter, District_or_City, ProvinceState, USA, Date, Creator
# and the offsets:      ^^^⁻4,^^^-3,   ^^^-2,            ^^^-1          ^^^  ^^^+1 ^^^+2
# so the 'info' field would be at index-4, the 'creator' field at index+2
#
# The output will be unconditionally cleaned of empties and uniquified unless you return a list of just one item
# (see the example at the end)
#
# this holds the item positions we want to drop
# dropping cannot be done ad hoc because it would shift the positions
def append_to_delete(delx, ix):
    if ix >= 0:
        delx.add(ix)


def has_indices(items, *indices):
    return all(0 <= ix < len(items) for ix in indices)


def modify(items, ix_to_check, val_to_check, ix_to_modify, val_to_modify, ix_to_delete=None, *, delx):
    # modify an item in items if it is not set depending on s/th matching another item,
    # optionally mark item for deletion
    #
    # if this item matches the control value, and
    if has_indices(items, ix_to_check, ix_to_modify) and items[ix_to_check] == val_to_check:
        # if the item to be modified is not set,
        if not items[ix_to_modify]:
            # modify it
            items[ix_to_modify] = val_to_modify
        # optionally mark item[ix] for deletion
        if ix_to_delete is not None and has_indices(items, ix_to_delete):
            append_to_delete(delx, ix_to_delete)


def pp_s_korea(items, it, ix, delx):
    # look for the item before the country ('South Korea'), it's ProvinceState
    # the structure is then Info, Quarter, District_or_City, ProvinceState, South Korea, Date, Creator
    # the offsets:          ^^^⁻4,^^^-3,   ^^^-2,            ^^^-1          ^^^we start here
    # the following assumes that the province suffix '-do' has already been regexed away
    #
    # ...in the big cities  and in Jeju, the name of the province is the well-known name, so keep it
    if has_indices(items, ix - 1) and items[ix - 1] not in ["Seoul", "Jeju", "Busan"]:
        # ...otherwise drop the province
        append_to_delete(delx, ix - 1)
    # cut away city quarter overkill
    quarter_parts = items[ix - 3].split(' ') if has_indices(items, ix - 3) else []
    if len(quarter_parts) > 1:
        items[ix - 3] = quarter_parts[0]
    # set some landmark names from the district quarter
    # TODO we could move this to a dict
    modify(items, ix - 3, 'Sanga', ix - 4, 'Woryeonggyo Bridge', ix - 3, delx=delx)
    modify(items, ix - 3, 'Pungcheon', ix - 4, 'Hahoe/Byeongsanseowon', ix - 3, delx=delx)
    modify(items, ix - 3, 'Jinhyeon', ix - 4, 'Bulguksa/Seokguram', ix - 3, delx=delx)
    modify(items, ix - 3, 'Cheongnyong', ix - 4, 'Beomeosa', ix - 3, delx=delx)
    return items


def pp_morocco(items, it, ix, delx):
    # drop the province, except when it's Marrake([s|c]h)
    if has_indices(items, ix - 1) and 'Marrakech' not in items[ix - 1]:
        append_to_delete(delx, ix - 1)
    # set some landmark names from the city
    modify(items, ix - 2, "M'Semrir", ix - 4, 'Gorges du Dades', delx=delx)
    modify(items, ix - 2, "Zerkten", ix - 4, "Tizi n'Tichka", delx=delx)
    modify(items, ix - 2, "Mezguita", ix - 4, "Tamnougalt", delx=delx)
    modify(items, ix - 2, "Ikniouen", ix - 4, "Jbel Saghro", delx=delx)

    return items


# cantons and abbreviations for them, to be extended
cantons = {'Zürich': 'ZH', 'Basel-Stadt': 'BS', 'St. Gallen': 'SG'}


# Someplace, Canton of Zürich, => Someplace ZH, unless Someplace in 'Canton of Zürich'
def pp_ch_cantons(items, ix, delx):
    if not has_indices(items, ix - 2, ix - 1) or not items[ix - 2]:
        return items
    ct = ''
    for canton in cantons.keys():
        # input fields
        input_canton = items[ix - 1]
        city = items[ix - 2]
        # if the dict term is in the input input_canton
        if canton in input_canton:
            # and if the city name is not part of the dict canton
            if city not in canton:
                # append the canton's abbreviation
                ct = ' ' + cantons.get(canton)
            # mark city and country field for deletion
            append_to_delete(delx, ix)
            append_to_delete(delx, ix - 2)
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
    # We assume that 'date' is the item before the last (len(items)-2)
    # and that 'country' is the item before date.
    # This has to be configured that way in the configuration file.
    # Invalid or missing dates leave the metadata unchanged.
    # This assumes the item list is configured like this:
    # ['info', 'sublocation', 'location', 'provincestate', 'country', 'date', 'creator']
    # We start at date, which is at len()-2                            ^^^-2
    # Our starting offset is 2, which gives us the position of date.
    i_start = 2
    i_pos = len(items) - i_start
    # We need at least country|date|creator, so more items than our starting offset.
    if len(items) <= i_start:
        return items
    # Get day, month, year from the configured ISO format or the legacy formats.
    # Each format must contain three numeric components with a four-digit year.
    date_formats = (
        (r'[0-9]{4}-[0-9]{1,2}-[0-9]{1,2}', '%Y-%m-%d'),
        (r'[0-9]{1,2}\.[0-9]{1,2}\.[0-9]{4}', '%d.%m.%Y'),
        (r'[0-9]{1,2}-[0-9]{1,2}-[0-9]{4}', '%m-%d-%Y'),
    )
    for pattern, date_format in date_formats:
        if re.fullmatch(pattern, items[i_pos]):
            try:
                # Validate the calendar date and convert it to sortable YYYYMMDD.
                i_date = datetime.strptime(items[i_pos], date_format).strftime('%Y%m%d')
            except ValueError:
                return items
            break
    else:
        return items
    # Check start and end dates inclusively, in earliest-start order.
    for timespan in sorted(timespans, key=lambda span: span[0]):
        start_date, end_date, *metadata = timespan
        if start_date <= i_date <= end_date:
            # We have a hit. Start at the item before date (the country).
            # Iterate the remaining tuple metadata toward the left of the item list.
            for offset, new_item in enumerate(metadata, start=1):
                target = i_pos - offset
                # Do not wrap around if the timespan contains too many metadata items.
                if target < 0:
                    break
                # Fill unset metadata only, leaving existing entries untouched.
                # Empty timespan values skip fields without changing them.
                if not items[target] and new_item:
                    items[target] = new_item
            # No more metadata from this tuple. The earliest matching entry wins.
            break
    return items
# consume the rest of the timespan after country

# for slides we delete all info that may have been set from the camera's GPS
def pp_dia(items, delx):
    for i in range(1, min(6, len(items) - 1)):
        append_to_delete(delx, i)


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
def postprocess(items: list[str], sep: str) -> list[str]:
    # Create a drop list for this invocation only.
    delx = set()
    # first, replace the global stuff
    items = pp_glob(items, glob_replacements)
    # get metadata from timespans
    # (if it was me who photographed)
    # if 'Hartmut' in items[len(items)-1]:
    items = pp_metadata_from_timespan(items)
    # now the specific filters
    if items and 'From Slide' in items[0]:
        pp_dia(items, delx)
    if len(items) >= 7:
        ix = len(items) - 3
        country = items[ix]
        if country == "South Korea":
            pp_s_korea(items, country, ix, delx)
        elif country == "Morocco":
            pp_morocco(items, country, ix, delx)
        elif country == "Switzerland":
            pp_ch_cantons(items, ix, delx)

    # only now, we remove the dropped items
    for x in sorted(delx, reverse=True):
        if has_indices(items, x):
            del items[x]

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
