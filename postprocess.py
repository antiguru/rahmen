# python code that takes the metadata tags, after they have
# been processed using the regex definitions and been joined by the separator,
# and processes them accordingly. It is currently required that this will return
# a list of items (strings).
#
# The output will be unconditionally cleaned of empties and uniquified (so you should probably
# set 'uniquify' and 'hide_empty' to false to have consistency in your input).
#
# this holds the item postions we want to drop
# dropping cannot be done ad hoc because it would shift the positions
delx = []


def pp_s_korea(items, it, ix):
    # look for the item before the country ('Südkorea'), it's ProvinceState
    # the structure is then Info, Quarter, District_or_City, ProvinceState, Südkorea, Date, Creator
    # the offsets:                ^^^-3    ^^^-2             ^^^-1     ^^^we start here
    # the following assumes that the province suffix '-do' has already been regexed away
    #
    # except in the case of Jeju, do this:
    if items[ix - 1] != "Jeju":
        # ...in the big cities, the name of the province is the well-known city name, so keep it
        if items[ix - 1] in ["Seoul", "Busan"]:
            # ...but drop the city district
            delx.append(ix - 2)
        else:
            # ...otherwise drop the province
            delx.append(ix - 1)
    # always drop the district quarter
    delx.append(ix - 3)
    return items


def pp_morocco(items, it, ix):
    # drop the province
    delx.append(ix - 1)
    return items


def pp_ch(items, it, ix):
    # Someplace, Kanton Zürich, => Someplace ZH,
    if items[ix - 1] == "Kanton Zürich":
        # we assign the new content to the province item
        items[ix - 1] = items[ix - 2] + ' ZH'
        # and we drop the location item
        delx.append(ix - 2)
    return items


def pp_mark(items, it, ix):
    # Someplace, Mark, => Someplace (Mark),
    # get location
    loc = items[ix - 1]
    # drop it
    delx.append(ix - 1)
    # assign new content to province item
    items[ix] = loc + ' ' + ''.join(['(', it, ')'])
    return items


# primitive global replacements: the dictionary has keys (to look up) and replacement values.
# these will be replaced wherever they occur
# only literal keys are allowed, no regular expressions.
def pp_glob(items):
    # value/replacement dictionary
    glob_replacements = {'Zurich': 'Zürich', ' City': '', ' Township': '', ' Province': ''}
    for i, it in enumerate(items):
        for k in glob_replacements.keys():
            # update the working value to prevent regressions when multiple matches occur
            it = it.replace(k, glob_replacements.get(k))
            items[i] = it
    return items


# main filter
def postprocess(items: [str], sep: str) -> str:
    outitems = []
    # clear the drop list
    delx.clear()
    print(items)
    # first, replace the global stuff
    items = pp_glob(items)
    print(items)
    for ix, it in enumerate(items):
        if it == "Südkorea":
            outitems = pp_s_korea(items, it, ix)
        if it == "Mark":
            outitems = pp_mark(items, it, ix)
        if it == "Marokko":
            outitems = pp_morocco(items, it, ix)
        if it == "Schweiz":
            outitems = pp_ch(items, it, ix)

    if not outitems:
        print("Status line unfiltered.")
    else:
        # only now, we remove the dropped items
        for x in delx:
            if x >= 0:
                if outitems[x]:
                    del outitems[x]
        print("Status line changed to:")
        print(outitems)
        items = outitems

    # if you return a list of items, the final processing will cause empties to be filtered out (unconditionally),
    # multiple items to be returned only once (if so configured), and finally join them to a line using the
    # separator string.
    return items
    # if you return a list of only one item, the final processing step will see this as a non-empty, unique, single item
    # and will return it unmodified. This way, you can define the final processing step here. The example drops
    # empty items and joins them with the separator, but does not remove multiples.
    # return [sep.join(filter(lambda x: len(x) > 0, items))]


def export():
    # return callable to Rust code
    return postprocess
