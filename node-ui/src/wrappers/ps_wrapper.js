// Copyright (c) 2017-2019, Substratum LLC (https://substratum.net) and/or its affiliates. All rights reserved.

module.exports = (() => {
  const ps = require('../main-process/ps')
  const treeKill = require('tree-kill')
  const path = require('path')

  async function killNodeProcess () {
    return ps().then(list => {
      list.filter(row =>
        (row.name.indexOf('SubstratumNode') >= 0 && row.cmd.indexOf('static' + path.sep + 'binaries') >= 0)
      ).forEach(item => treeKill(item.pid))
    })
  }

  async function findNodeProcess () {
    return ps().then(list => {
      return list.filter(row =>
        (row.name.indexOf('SubstratumNode') >= 0 && row.cmd.indexOf('static' + path.sep + 'binaries') >= 0)
      )
    })
  }

  return {
    killNodeProcess: killNodeProcess,
    findNodeProcess: findNodeProcess
  }
})()
