# python code that takes the metadata tags, after they have
# been processed using the regex definitions and been joined by the separator,
# and processes them accordingly. It is currently required that this will return
# a list of items (strings).
#
# The output will be unconditionally cleaned of empties and uniquified (so you should probably
# set 'uniquify' and 'hide_empty' to false to have consistency in your input).
#
# There must be at least one function and this function has to be named 'postprocess'.
# It has two positional arguments, the first is the text string to process, the second is
# the separator string. The return has to be a list of strings.
# So the simplest code, which takes the input, splits it and returns the list, would be
#
# def postprocess(text,sep):
#   return text.split(sep)
#
# Be aware that this might seem to do effectively nothing, because the returned list will be joined
# using the separator. If the input should differ from the output, that will be the result of the
# unconditional deduplication and removal of empty items that the returned list undergoes
# before it is joined to the final output line.

delx = []

def pp_s_korea(items,i):
    if items[items.index(i) - 1] in [ "Seoul", "Busan" ]:
        delx.append(items.index(i) - 2)
    else:
        delx.append(items.index(i) -1)
    if items[items.index(i) -1] != "Jeju":
        delx.append(items.index(i) -3)
    return items

def pp_morocco(items,i):
    delx.append(items.index(i) -1)
    return items

def pp_ch(items,i):
    mi = items.index(i)
    if items[mi-1] == "Kanton Zürich":
        items[mi-1] = items[mi-2] + ' ZH'
        delx.append(mi-2)
    return items

def pp_mark(items,i):
    mi = items.index(i)
    loc = items[mi-1]
    delx.append(mi-1)
    items[mi] = loc + ' ' + ''.join(['(', i, ')'])
    return items


def postprocess(text, sep):
    outitems = []
    delx.clear()
    items = text.split(sep)
    print(items)
    for i in items:
        if i == "Südkorea":
            outitems = pp_s_korea(items,i)
        if i == "Mark":
            outitems = pp_mark(items,i)
        if i == "Marokko":
            outitems = pp_morocco(items,i)
        if i == "Schweiz":
            outitems = pp_ch(items,i)

    if not outitems:
        print("Status line unchanged.")
        return items
    else:
        for x in delx:
            if x >= 0:
                del outitems[x]
        print("Status line changed to:")
        print(outitems)
        return outitems

def export():
    return postprocess
