#!/usr/bin/env python

from setuptools import setup

setup(name='OpenSRF',
    version='1.0.0',
    install_requires=[
        'dnspython', # required by pyxmpp
    	'python-memcached', # older setuptools fails to resolve this
        'pyxmpp>=1.0.0',
        'simplejson>=1.7.1'
    ],
    dependency_links = [
        "http://pyxmpp.jajcus.net/downloads/",
    ],
    description='OpenSRF Python Modules',
    author='Bill Erickson',
    author_email='erickson@esilibrary.com',
    license="GPL",
    url='http://www.open-ils.org/',
    packages=['osrf'],
    scripts=['srfsh.py']
)
