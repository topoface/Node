// Copyright (c) 2017-2019, Substratum LLC (https://substratum.net) and/or its affiliates. All rights reserved.
const { dialog } = require('electron')

const childProcess = require('child_process')
const path = require('path')
const consoleWrapper = require('../wrappers/console_wrapper')
const dnsUtility = require('./dns_utility')
const psWrapper = require('../wrappers/ps_wrapper')
const uiInterface = require('./ui_interface')
const NODE_STARTUP_TIMEOUT = 60000
const NODE_SHUTDOWN_TIMEOUT = 5000

module.exports = class NodeActuator {
  constructor (webContents) {
    this.webContents = webContents
  }

  async setStatusToOffThenRevert () {
    this.substratumNodeProcess = null

    await dnsUtility.revert()
    return this.setStatus()
  }

  bindProcessEvents () {
    this.substratumNodeProcess.on('message', message => {
      consoleWrapper.log('substratum_node process received message: ', message)
      if (message.startsWith('Command returned error: ')) {
        // TODO: SC-680 says to make Node terminations stop triggering this line and uncomment it.
        // if (substratumNodeProcess) { dialog.showErrorBox('Error', message) }
        return this.setStatusToOffThenRevert()
      } else {
        return Promise.resolve(null)
      }
    })

    this.substratumNodeProcess.on('error', async error => {
      consoleWrapper.log('substratum_node process received error: ', error.message)
      if (this.substratumNodeProcess) { dialog.showErrorBox('Error', error.message) }
      return this.setStatusToOffThenRevert()
    })

    this.substratumNodeProcess.on('exit', async code => {
      consoleWrapper.log('substratum_node process exited with code ', code)
      this.substratumNodeProcess = null
      await this.setStatus()
    })
  }

  async getStatus () {
    return this.determineStatus(await psWrapper.findNodeProcess())
  }

  async setStatus () {
    let processList = await psWrapper.findNodeProcess()
    let status = await this.determineStatus(processList)
    if (status === 'Consuming' || status === 'Serving') {
      this.substratumNodeProcess = processList[0]
    } else {
      this.substratumNodeProcess = null
    }
    return this.webContents.send('node-status', status)
  }

  determineStatus (processList) {
    let dnsStatus = dnsUtility.getStatus()
    if (processList && processList.length > 0 && dnsStatus.indexOf('subverted') >= 0) {
      return 'Consuming'
    } else if (processList && processList.length > 0) {
      return 'Serving'
    } else if (dnsStatus.indexOf('subverted') >= 0) {
      return 'Invalid'
    } else {
      return 'Off'
    }
  }

  async startNode () {
    if (this.substratumNodeProcess) {
      await uiInterface.connect()
      return
    }

    const worker = path.resolve(__dirname, '.', './substratum_node.js')
    this.substratumNodeProcess = childProcess.fork(worker, [], {
      silent: true,
      stdio: [0, 1, 2, 'ipc'],
      detached: true
    })
    this.bindProcessEvents()
    this.substratumNodeProcess.send('start')

    if (await uiInterface.verifyNodeUp(NODE_STARTUP_TIMEOUT)) {
      await uiInterface.connect()
    } else {
      dialog.showErrorBox('Error', `Node was started but didn't come up within ${NODE_STARTUP_TIMEOUT}ms!`)
    }
  }

  async stopNode () {
    if (uiInterface.isConnected()) {
      uiInterface.shutdown()
    }
    if (!(await uiInterface.verifyNodeDown(NODE_SHUTDOWN_TIMEOUT))) {
      if (!this.substratumNodeProcess || this.substratumNodeProcess.cmd) {
        // Don't have a handle to the process; have to pkill
        this.substratumNodeProcess = null
        return psWrapper.killNodeProcess()
      } else {
        // Have a handle to the process; can send a 'stop' message
        let processToKill = this.substratumNodeProcess
        this.substratumNodeProcess = null
        processToKill.send('stop')
        return Promise.resolve(null)
      }
    }
  }

  async offClick () {
    try {
      await dnsUtility.revert()
      await this.stopNode()
      return this.getStatus()
    } catch (error) {
      dialog.showErrorBox('Error', error.message)
      return this.getStatus()
    }
  }

  async servingClick () {
    await this.startNode()
    try {
      await dnsUtility.revert()
      return await this.getStatus()
    } catch (error) {
      dialog.showErrorBox('Error', error.message)
      return this.getStatus()
    }
  }

  async consumingClick () {
    await this.startNode()
    try {
      await dnsUtility.subvert()
      return await this.getStatus()
    } catch (error) {
      dialog.showErrorBox('Error', error.message)
      return await this.getStatus()
    }
  }

  async shutdown () {
    return dnsUtility.revert().then(
      () => this.stopNode(),
      (error) => {
        dialog.showErrorBox('Error', `Couldn't stop consuming: ${error.message}`)
      }
    )
  }
}
